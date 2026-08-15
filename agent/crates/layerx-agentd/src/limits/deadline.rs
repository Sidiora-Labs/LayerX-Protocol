use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// An absolute finite lifetime required for every daemon request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RequestDeadline {
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

impl RequestDeadline {
    pub fn new(started_at_ms: u64, expires_at_ms: u64) -> Result<Self, DeadlineError> {
        if expires_at_ms <= started_at_ms {
            Err(DeadlineError::InvalidDeadline)
        } else {
            Ok(Self {
                started_at_ms,
                expires_at_ms,
            })
        }
    }

    /// Bounds one downstream call by both its own timeout and the request lifetime.
    pub fn boundary_call(
        self,
        observed_at_ms: u64,
        maximum_duration_ms: u64,
    ) -> Result<BoundaryDeadline, DeadlineError> {
        if maximum_duration_ms == 0 || observed_at_ms < self.started_at_ms {
            return Err(DeadlineError::InvalidDeadline);
        }
        if observed_at_ms >= self.expires_at_ms {
            return Err(DeadlineError::Elapsed);
        }
        let call_expiry = observed_at_ms
            .checked_add(maximum_duration_ms)
            .ok_or(DeadlineError::Arithmetic)?;
        Ok(BoundaryDeadline {
            started_at_ms: observed_at_ms,
            expires_at_ms: call_expiry.min(self.expires_at_ms),
        })
    }
}

/// Finite lifetime for one core, signer, or storage boundary call.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundaryDeadline {
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

/// A real cancellation signal held by downstream work.
#[derive(Clone, Debug)]
pub struct CancellationToken(Arc<AtomicBool>);

impl CancellationToken {
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    fn mark_cancelled(&self) {
        self.0.store(true, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteStage {
    Preparing,
    Signing,
    DurableQueued,
    Transmitting,
    Acknowledged,
    UnknownResolving,
}

impl WriteStage {
    const fn ordinal(self) -> u8 {
        match self {
            Self::Preparing => 0,
            Self::Signing => 1,
            Self::DurableQueued => 2,
            Self::Transmitting => 3,
            Self::Acknowledged => 4,
            Self::UnknownResolving => 5,
        }
    }

    const fn may_have_reached_core(self) -> bool {
        self.ordinal() >= Self::Transmitting.ordinal()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkOwner {
    Caller,
    DaemonResolver,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrackedWork {
    Read,
    Write {
        submission_id: [u8; 32],
        stage: WriteStage,
        reservation_held: bool,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisconnectOutcome {
    Cancelled,
    ResolutionContinues { submission_id: [u8; 32] },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineOutcome {
    Cancelled {
        request_id: u64,
    },
    ReportedUnknown {
        request_id: u64,
        submission_id: [u8; 32],
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AgeDistribution {
    pub under_one_second: usize,
    pub under_ten_seconds: usize,
    pub under_one_minute: usize,
    pub at_least_one_minute: usize,
    pub oldest_ms: u64,
}

impl AgeDistribution {
    fn record(&mut self, age_ms: u64) {
        self.oldest_ms = self.oldest_ms.max(age_ms);
        match age_ms {
            0..=999 => self.under_one_second += 1,
            1_000..=9_999 => self.under_ten_seconds += 1,
            10_000..=59_999 => self.under_one_minute += 1,
            _ => self.at_least_one_minute += 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkMetrics {
    pub in_flight: usize,
    pub unresolved: usize,
    pub in_flight_age: AgeDistribution,
    pub unresolved_age: AgeDistribution,
}

#[derive(Clone, Debug)]
struct TrackedRequest {
    deadline: RequestDeadline,
    work: TrackedWork,
    owner: WorkOwner,
    caller_connected: bool,
    unknown_since_ms: Option<u64>,
    cancellation: CancellationToken,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkView {
    pub deadline: RequestDeadline,
    pub work: TrackedWork,
    pub owner: WorkOwner,
    pub caller_connected: bool,
    pub unknown_since_ms: Option<u64>,
    pub cancelled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeadlineError {
    InvalidDeadline,
    Elapsed,
    DuplicateRequest(u64),
    UnknownRequest(u64),
    InvalidTransition,
    TimeRegressed,
    Arithmetic,
    OrphanRisk,
}

/// Owns request deadlines, cancellation signals, and detached unknown resolvers.
#[derive(Debug, Default)]
pub struct RequestTracker {
    requests: BTreeMap<u64, TrackedRequest>,
}

impl RequestTracker {
    pub fn begin_read(
        &mut self,
        request_id: u64,
        deadline: RequestDeadline,
    ) -> Result<CancellationToken, DeadlineError> {
        self.insert(request_id, deadline, TrackedWork::Read)
    }

    pub fn begin_write(
        &mut self,
        request_id: u64,
        submission_id: [u8; 32],
        deadline: RequestDeadline,
    ) -> Result<CancellationToken, DeadlineError> {
        self.insert(
            request_id,
            deadline,
            TrackedWork::Write {
                submission_id,
                stage: WriteStage::Preparing,
                reservation_held: true,
            },
        )
    }

    pub fn advance_write(
        &mut self,
        request_id: u64,
        stage: WriteStage,
        observed_at_ms: u64,
    ) -> Result<(), DeadlineError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(DeadlineError::UnknownRequest(request_id))?;
        if observed_at_ms < request.deadline.started_at_ms {
            return Err(DeadlineError::TimeRegressed);
        }
        let TrackedWork::Write { stage: current, .. } = &mut request.work else {
            return Err(DeadlineError::InvalidTransition);
        };
        if stage.ordinal() != current.ordinal() + 1 {
            return Err(DeadlineError::InvalidTransition);
        }
        *current = stage;
        if stage == WriteStage::UnknownResolving {
            request.owner = WorkOwner::DaemonResolver;
            request.unknown_since_ms.get_or_insert(observed_at_ms);
        }
        Ok(())
    }

    /// Removes completed cancellable work. Unknown submissions require receipt resolution.
    pub fn complete(&mut self, request_id: u64) -> Result<(), DeadlineError> {
        let request = self
            .requests
            .get(&request_id)
            .ok_or(DeadlineError::UnknownRequest(request_id))?;
        if matches!(
            request.work,
            TrackedWork::Write {
                stage: WriteStage::Transmitting
                    | WriteStage::Acknowledged
                    | WriteStage::UnknownResolving,
                ..
            }
        ) {
            return Err(DeadlineError::OrphanRisk);
        }
        self.requests.remove(&request_id);
        Ok(())
    }

    /// Removes an unresolved write only after receipt-only resolution reaches terminal state.
    pub fn resolved_by_receipt(&mut self, request_id: u64) -> Result<(), DeadlineError> {
        let request = self
            .requests
            .get(&request_id)
            .ok_or(DeadlineError::UnknownRequest(request_id))?;
        if !matches!(
            request.work,
            TrackedWork::Write {
                stage: WriteStage::UnknownResolving,
                ..
            }
        ) {
            return Err(DeadlineError::InvalidTransition);
        }
        self.requests.remove(&request_id);
        Ok(())
    }

    pub fn expire(&mut self, observed_at_ms: u64) -> Result<Vec<DeadlineOutcome>, DeadlineError> {
        let expired = self
            .requests
            .iter()
            .filter_map(|(request_id, request)| {
                (request.caller_connected && observed_at_ms >= request.deadline.expires_at_ms)
                    .then_some(*request_id)
            })
            .collect::<Vec<_>>();
        let mut outcomes = Vec::with_capacity(expired.len());
        for request_id in expired {
            let outcome = self.disconnect(request_id, observed_at_ms)?;
            outcomes.push(match outcome {
                DisconnectOutcome::Cancelled => DeadlineOutcome::Cancelled { request_id },
                DisconnectOutcome::ResolutionContinues { submission_id } => {
                    DeadlineOutcome::ReportedUnknown {
                        request_id,
                        submission_id,
                    }
                }
            });
        }
        Ok(outcomes)
    }

    pub fn metrics(&self, observed_at_ms: u64) -> Result<WorkMetrics, DeadlineError> {
        let mut metrics = WorkMetrics {
            in_flight: self.requests.len(),
            ..WorkMetrics::default()
        };
        for request in self.requests.values() {
            if observed_at_ms < request.deadline.started_at_ms {
                return Err(DeadlineError::TimeRegressed);
            }
            metrics
                .in_flight_age
                .record(observed_at_ms - request.deadline.started_at_ms);
            if let Some(unknown_since_ms) = request.unknown_since_ms {
                if observed_at_ms < unknown_since_ms {
                    return Err(DeadlineError::TimeRegressed);
                }
                metrics.unresolved += 1;
                metrics
                    .unresolved_age
                    .record(observed_at_ms - unknown_since_ms);
            }
        }
        Ok(metrics)
    }

    #[must_use]
    pub fn view(&self, request_id: u64) -> Option<WorkView> {
        self.requests.get(&request_id).map(|request| WorkView {
            deadline: request.deadline,
            work: request.work.clone(),
            owner: request.owner,
            caller_connected: request.caller_connected,
            unknown_since_ms: request.unknown_since_ms,
            cancelled: request.cancellation.is_cancelled(),
        })
    }

    fn insert(
        &mut self,
        request_id: u64,
        deadline: RequestDeadline,
        work: TrackedWork,
    ) -> Result<CancellationToken, DeadlineError> {
        if self.requests.contains_key(&request_id) {
            return Err(DeadlineError::DuplicateRequest(request_id));
        }
        let cancellation = CancellationToken(Arc::new(AtomicBool::new(false)));
        self.requests.insert(
            request_id,
            TrackedRequest {
                deadline,
                work,
                owner: WorkOwner::Caller,
                caller_connected: true,
                unknown_since_ms: None,
                cancellation: cancellation.clone(),
            },
        );
        Ok(cancellation)
    }

    fn disconnect(
        &mut self,
        request_id: u64,
        observed_at_ms: u64,
    ) -> Result<DisconnectOutcome, DeadlineError> {
        let request = self
            .requests
            .get_mut(&request_id)
            .ok_or(DeadlineError::UnknownRequest(request_id))?;
        if observed_at_ms < request.deadline.started_at_ms {
            return Err(DeadlineError::TimeRegressed);
        }
        request.caller_connected = false;
        let submission = match &mut request.work {
            TrackedWork::Read => None,
            TrackedWork::Write {
                submission_id,
                stage,
                reservation_held,
            } if stage.may_have_reached_core() => {
                *stage = WriteStage::UnknownResolving;
                *reservation_held = true;
                Some(*submission_id)
            }
            TrackedWork::Write { .. } => None,
        };
        if let Some(submission_id) = submission {
            request.owner = WorkOwner::DaemonResolver;
            request.unknown_since_ms.get_or_insert(observed_at_ms);
            Ok(DisconnectOutcome::ResolutionContinues { submission_id })
        } else {
            request.cancellation.mark_cancelled();
            self.requests.remove(&request_id);
            Ok(DisconnectOutcome::Cancelled)
        }
    }
}

/// Cancels caller-owned downstream work or transfers an indeterminate submission
/// to the daemon's receipt resolver.
pub fn disconnect_request(
    tracker: &mut RequestTracker,
    request_id: u64,
    observed_at_ms: u64,
) -> Result<DisconnectOutcome, DeadlineError> {
    tracker.disconnect(request_id, observed_at_ms)
}

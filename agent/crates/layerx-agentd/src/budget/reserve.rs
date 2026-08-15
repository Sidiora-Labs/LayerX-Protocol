//! Atomic multi-scope budget reservations.

use std::collections::BTreeMap;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LimitId(pub [u8; 16]);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitScope {
    Tenant([u8; 32]),
    Agent([u8; 32]),
    Session([u8; 32]),
    Capability([u8; 32]),
    Counterparty([u8; 32]),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LimitConfig {
    pub id: LimitId,
    pub name: String,
    pub scope: LimitScope,
    pub ceiling: u128,
    pub consumed: u128,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LimitState {
    config: LimitConfig,
    held: BTreeMap<[u8; 32], (u128, u64)>,
}

#[derive(Debug)]
pub struct BudgetLimiter {
    limits: Mutex<BTreeMap<LimitId, LimitState>>,
}

impl BudgetLimiter {
    pub fn new(configs: Vec<LimitConfig>) -> Result<Self, LimitRefusal> {
        let mut limits = BTreeMap::new();
        for config in configs {
            if config.ceiling == 0 || config.consumed > config.ceiling {
                return Err(LimitRefusal::InvalidConfiguration);
            }
            if limits
                .insert(
                    config.id,
                    LimitState {
                        config,
                        held: BTreeMap::new(),
                    },
                )
                .is_some()
            {
                return Err(LimitRefusal::InvalidConfiguration);
            }
        }
        Ok(Self {
            limits: Mutex::new(limits),
        })
    }

    pub fn held_reservations(&self) -> Result<usize, LimitRefusal> {
        let limits = self.limits.lock().map_err(|_| LimitRefusal::Poisoned)?;
        Ok(limits.values().map(|limit| limit.held.len()).sum())
    }

    pub fn consumed(&self, id: LimitId) -> Result<u128, LimitRefusal> {
        let limits = self.limits.lock().map_err(|_| LimitRefusal::Poisoned)?;
        limits
            .get(&id)
            .map(|limit| limit.config.consumed)
            .ok_or(LimitRefusal::UnknownLimit(id))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReservationRequest {
    pub id: [u8; 32],
    pub amount: u128,
    pub expiry_sequence: u64,
    pub current_sequence: u64,
    pub applicable_limits: Vec<LimitId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BudgetReservation {
    pub id: [u8; 32],
    pub amount: u128,
    pub applied_limits: Vec<LimitId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseKind {
    Executed,
    Failed,
    Expired,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LimitRefusal {
    Exceeded {
        limit: LimitId,
        name: String,
        ceiling: u128,
        consumed: u128,
        held: u128,
        requested: u128,
    },
    UnknownLimit(LimitId),
    InvalidConfiguration,
    InvalidRequest,
    Arithmetic,
    Poisoned,
}

pub(crate) fn reserve_all(
    limiter: &BudgetLimiter,
    request: &ReservationRequest,
) -> Result<BudgetReservation, LimitRefusal> {
    if request.amount == 0
        || request.expiry_sequence <= request.current_sequence
        || request.applicable_limits.is_empty()
    {
        return Err(LimitRefusal::InvalidRequest);
    }
    let mut applicable = request.applicable_limits.clone();
    applicable.sort_unstable();
    applicable.dedup();
    let mut limits = limiter.limits.lock().map_err(|_| LimitRefusal::Poisoned)?;
    for id in &applicable {
        let limit = limits.get(id).ok_or(LimitRefusal::UnknownLimit(*id))?;
        let held = held_total(limit)?;
        let projected = limit
            .config
            .consumed
            .checked_add(held)
            .and_then(|value| value.checked_add(request.amount))
            .ok_or(LimitRefusal::Arithmetic)?;
        if projected > limit.config.ceiling {
            return Err(LimitRefusal::Exceeded {
                limit: *id,
                name: limit.config.name.clone(),
                ceiling: limit.config.ceiling,
                consumed: limit.config.consumed,
                held,
                requested: request.amount,
            });
        }
    }
    for id in &applicable {
        if let Some(limit) = limits.get_mut(id) {
            limit
                .held
                .insert(request.id, (request.amount, request.expiry_sequence));
        }
    }
    Ok(BudgetReservation {
        id: request.id,
        amount: request.amount,
        applied_limits: applicable,
    })
}

pub(crate) fn release_all(
    limiter: &BudgetLimiter,
    reservation_id: [u8; 32],
    kind: ReleaseKind,
    current_sequence: u64,
) -> Result<bool, LimitRefusal> {
    if kind == ReleaseKind::Unknown {
        return Ok(false);
    }
    let mut limits = limiter.limits.lock().map_err(|_| LimitRefusal::Poisoned)?;
    let mut found = false;
    for limit in limits.values_mut() {
        let Some((amount, expiry)) = limit.held.get(&reservation_id).copied() else {
            continue;
        };
        if kind == ReleaseKind::Expired && current_sequence < expiry {
            continue;
        }
        limit.held.remove(&reservation_id);
        if kind == ReleaseKind::Executed {
            limit.config.consumed = limit
                .config
                .consumed
                .checked_add(amount)
                .ok_or(LimitRefusal::Arithmetic)?;
        }
        found = true;
    }
    Ok(found)
}

fn held_total(limit: &LimitState) -> Result<u128, LimitRefusal> {
    limit
        .held
        .values()
        .try_fold(0_u128, |total, (amount, _)| total.checked_add(*amount))
        .ok_or(LimitRefusal::Arithmetic)
}

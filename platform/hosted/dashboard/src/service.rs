//! The developer dashboard read model.
//!
//! One instance reads two durable stores it does not own: the hosted gateway's
//! store, for keys, quotas, usage, the request log and retained receipts, and
//! the hosted webhook store, for endpoint health, delivery logs, the dead-letter
//! path and the payment events the platform actually delivered. Nothing here
//! writes, dispatches or issues; every method is a projection of what those two
//! services already recorded, scoped to the calling principal.

use std::path::Path;

use layerx_platform_webhooks::deliveries::DeliveryRecord;
use layerx_platform_webhooks::endpoints::{EndpointHealth, RetryPolicy};
use layerx_platform_webhooks::events::{EndpointId, EventKind, Principal};
use layerx_platform_webhooks::state::Ledger;

use crate::error::DashboardError;
use crate::gateway::Store;
use crate::model::{
    DeliverySummary, KeyView, Overview, PaymentView, ReceiptView, RequestRecord, UsageSummary,
};

/// Largest page any dashboard view returns.
pub const MAXIMUM_PAGE: usize = 200;
/// Rows each list in the landing view carries.
pub const OVERVIEW_PAGE: usize = 20;

/// The developer dashboard over one gateway store and one webhook store.
pub struct Dashboard {
    gateway: Store,
    ledger: Ledger,
}

impl Dashboard {
    /// Opens both durable stores for reading.
    ///
    /// # Errors
    /// Returns [`DashboardError::UnknownRoot`] when either root is not an
    /// existing directory and [`DashboardError::Webhooks`] when the retry
    /// contract the webhook health projection needs is unusable.
    pub fn open(
        gateway_root: impl AsRef<Path>,
        webhook_root: impl AsRef<Path>,
        policy: RetryPolicy,
    ) -> Result<Self, DashboardError> {
        let webhook_root = webhook_root.as_ref();
        if !webhook_root.is_dir() {
            return Err(DashboardError::UnknownRoot);
        }
        Ok(Self {
            gateway: Store::open(gateway_root)?,
            ledger: Ledger::open(webhook_root, policy)?,
        })
    }

    /// Returns the API keys the principal holds.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] or [`DashboardError::Io`] when
    /// the gateway store cannot be read.
    pub fn keys(&self, principal: &Principal, now: u64) -> Result<Vec<KeyView>, DashboardError> {
        self.gateway.keys(principal, now)
    }

    /// Returns quota and usage across those keys.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] or [`DashboardError::Io`] when
    /// the gateway store cannot be read.
    pub fn usage(&self, principal: &Principal, now: u64) -> Result<UsageSummary, DashboardError> {
        self.gateway.usage(principal, now)
    }

    /// Returns the principal's own request log, newest first.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`] or [`DashboardError::Io`] when
    /// the gateway store cannot be read.
    pub fn requests(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<RequestRecord>, DashboardError> {
        self.gateway.requests(principal, limit.min(MAXIMUM_PAGE))
    }

    /// Returns the receipt retained under one of the developer's own
    /// idempotency keys.
    ///
    /// # Errors
    /// Returns [`DashboardError::InvalidRequest`] for an unusable key,
    /// [`DashboardError::UnknownReceipt`] when none is recorded under it, and
    /// [`DashboardError::CorruptStore`] or [`DashboardError::Io`] when the
    /// gateway store cannot be read.
    pub fn receipt(
        &self,
        principal: &Principal,
        idempotency_key: &str,
    ) -> Result<ReceiptView, DashboardError> {
        self.gateway.receipt(principal, idempotency_key)
    }

    /// Returns webhook delivery health for every endpoint the principal owns.
    ///
    /// # Errors
    /// Returns [`DashboardError::Webhooks`] when the webhook store cannot be
    /// read or decoded.
    pub fn endpoints(
        &self,
        principal: &Principal,
        now: u64,
    ) -> Result<Vec<EndpointHealth>, DashboardError> {
        Ok(self.ledger.health(principal, now)?)
    }

    /// Returns the webhook delivery log, newest first.
    ///
    /// # Errors
    /// Returns [`DashboardError::Webhooks`] when the webhook store cannot be
    /// read or decoded.
    pub fn deliveries(
        &self,
        principal: &Principal,
        endpoint: Option<&EndpointId>,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, DashboardError> {
        Ok(self
            .ledger
            .deliveries(principal, endpoint, limit.min(MAXIMUM_PAGE))?)
    }

    /// Returns the dead-letter path, newest first.
    ///
    /// # Errors
    /// Returns [`DashboardError::Webhooks`] when the webhook store cannot be
    /// read or decoded.
    pub fn dead_letters(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<DeliveryRecord>, DashboardError> {
        Ok(self
            .ledger
            .dead_letters(principal, limit.min(MAXIMUM_PAGE))?)
    }

    /// Returns the test payments the platform delivered for this principal,
    /// newest first, each carrying the evidence behind every fact it displays.
    ///
    /// # Errors
    /// Returns [`DashboardError::Webhooks`] when the webhook store cannot be
    /// read or decoded.
    pub fn payments(
        &self,
        principal: &Principal,
        limit: usize,
    ) -> Result<Vec<PaymentView>, DashboardError> {
        Ok(self
            .ledger
            .events(principal, Some(EventKind::Payment), limit.min(MAXIMUM_PAGE))?
            .iter()
            .map(PaymentView::of)
            .collect())
    }

    /// Assembles the developer landing view in one pass over both stores.
    ///
    /// # Errors
    /// Returns [`DashboardError::CorruptStore`], [`DashboardError::Io`] or
    /// [`DashboardError::Webhooks`] when either store cannot be read.
    pub fn overview(&self, principal: &Principal, now: u64) -> Result<Overview, DashboardError> {
        let snapshot = self.gateway.snapshot(principal, now, OVERVIEW_PAGE)?;
        let endpoints = self.ledger.health(principal, now)?;
        Ok(Overview {
            principal: principal.as_str().to_owned(),
            generated_at: now,
            usage: snapshot.usage,
            keys: snapshot.keys,
            requests: snapshot.requests,
            recent_requests: snapshot.recent_requests,
            deliveries: DeliverySummary::of(&endpoints),
            endpoints,
            dead_letters: self.ledger.dead_letters(principal, OVERVIEW_PAGE)?,
            payments: self.payments(principal, OVERVIEW_PAGE)?,
        })
    }
}

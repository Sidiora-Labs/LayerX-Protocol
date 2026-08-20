use std::time::Duration;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestnetConfig {
    pub protocol_version: String,
    pub network_id: u32,
    pub public_endpoint: String,
    pub gateway_endpoint: String,
    pub reset_schedule: String,
    pub snapshot_interval: Duration,
}

impl TestnetConfig {
    /// Verifies that hosted configuration is safe to deploy for the pending release.
    ///
    /// # Errors
    ///
    /// Returns an error when the version differs or an operational invariant is invalid.
    pub fn validate(&self, pending_release: &str) -> Result<(), &'static str> {
        if self.protocol_version != pending_release {
            return Err("testnet protocol version does not match pending release");
        }
        if self.network_id == 0
            || !self.public_endpoint.starts_with("https://")
            || !self.gateway_endpoint.starts_with("https://")
            || self.reset_schedule.is_empty()
            || self.snapshot_interval.is_zero()
        {
            return Err("invalid testnet operations configuration");
        }
        Ok(())
    }
}

#[must_use]
pub fn platform_testnet() -> TestnetConfig {
    TestnetConfig {
        protocol_version: env!("CARGO_PKG_VERSION").to_string(),
        network_id: 402,
        public_endpoint: "https://testnet.layerx.network".to_string(),
        gateway_endpoint: "https://api.testnet.layerx.network".to_string(),
        reset_schedule: "09:00 UTC on the first Tuesday of every month".to_string(),
        snapshot_interval: Duration::from_secs(15),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declared_testnet_matches_crate_release() {
        let config = platform_testnet();
        assert_eq!(config.validate(env!("CARGO_PKG_VERSION")), Ok(()));
        assert!(config.validate("next-release").is_err());
    }
}

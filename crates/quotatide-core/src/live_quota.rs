use serde::Serialize;
use ts_rs::TS;

/// Millionths of one percentage point. `100% == 100_000_000`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuotaUnits(i64);

impl QuotaUnits {
    pub const FULL: Self = Self(100_000_000);

    #[must_use]
    pub const fn from_micropoints(value: i64) -> Option<Self> {
        if value >= 0 && value <= Self::FULL.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn micropoints(self) -> i64 {
        self.0
    }
}

/// Strictly normalized current seven-day quota observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeeklyUsageObservation {
    pub captured_at_unix_ms: i64,
    pub used: QuotaUnits,
    pub window_seconds: u32,
    pub resets_at_unix_s: i64,
    pub plan_type: Option<String>,
    pub allowed: Option<bool>,
}

/// Stable source failure categories; raw upstream content never crosses this boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum UsageSourceErrorCode {
    AuthenticationStale,
    PermissionDenied,
    RateLimited,
    Timeout,
    UpstreamUnavailable,
    ResponseTooLarge,
    InvalidJson,
    ContractViolation,
    WeeklyWindowUnavailable,
}

/// Freshness state projected alongside the last-known-good observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum SourceFreshness {
    Fresh,
    Stale,
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::QuotaUnits;

    #[test]
    fn quota_units_are_bounded_and_exact() {
        assert_eq!(
            QuotaUnits::from_micropoints(41_250_000)
                .expect("valid units")
                .micropoints(),
            41_250_000
        );
        assert!(QuotaUnits::from_micropoints(-1).is_none());
        assert!(QuotaUnits::from_micropoints(100_000_001).is_none());
    }
}

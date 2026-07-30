use serde::{Deserialize, Serialize};
use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use ts_rs::TS;

use crate::SourceStatus;

const SOURCE_ACCOUNT: &str = "thsottiaux";
const MAX_TEXT_BYTES: usize = 4_096;

/// Exact reset probability in one-hundredths of a percentage point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RadarChance(u16);

impl RadarChance {
    pub const FULL: u16 = 10_000;

    #[must_use]
    pub const fn from_basis_points(value: u16) -> Option<Self> {
        if value <= Self::FULL {
            Some(Self(value))
        } else {
            None
        }
    }

    #[must_use]
    pub const fn basis_points(self) -> u16 {
        self.0
    }
}

/// Source-compatible probability bucket that avoids presenting false precision.
#[must_use]
pub fn radar_bucket_label(chance: RadarChance) -> &'static str {
    match chance.basis_points() {
        0..=999 => "可能暗示额外重置",
        1_000..=1_999 => ">10%",
        2_000..=2_999 => ">20%",
        3_000..=3_999 => ">30%",
        4_000..=4_999 => ">40%",
        5_000..=5_999 => ">50%",
        6_000..=6_999 => ">60%",
        7_000..=7_999 => ">70%",
        8_000..=8_999 => ">80%",
        _ => ">90%",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RadarContractError {
    #[error("radar source identifier is invalid")]
    InvalidSourceId,
    #[error("radar source URL is not the approved post URL")]
    UnsafeSourceUrl,
    #[error("radar time range is invalid")]
    InvalidTimeRange,
    #[error("radar explanation is invalid")]
    InvalidExplanation,
}

/// Strictly normalized current third-party reset prediction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarObservation {
    source_id: String,
    chance: RadarChance,
    observed_at_unix_ms: i64,
    expires_at_unix_ms: i64,
    explanation: String,
    source_url: String,
}

impl RadarObservation {
    /// Validates one prediction without retaining the upstream response.
    ///
    /// # Errors
    ///
    /// Rejects unsafe source links, invalid times, IDs, or explanation text.
    pub fn new(
        source_id: impl Into<String>,
        chance_basis_points: u16,
        observed_at_unix_ms: i64,
        expires_at_unix_ms: i64,
        explanation: impl Into<String>,
        source_url: impl Into<String>,
    ) -> Result<Self, RadarContractError> {
        let source_id = source_id.into();
        validate_source_id(&source_id)?;
        let source_url = source_url.into();
        validate_source_url(&source_url, &source_id)?;
        if observed_at_unix_ms <= 0 || expires_at_unix_ms <= observed_at_unix_ms {
            return Err(RadarContractError::InvalidTimeRange);
        }
        let explanation = explanation.into();
        validate_text(&explanation)?;
        let chance = RadarChance::from_basis_points(chance_basis_points)
            .ok_or(RadarContractError::InvalidExplanation)?;
        Ok(Self {
            source_id,
            chance,
            observed_at_unix_ms,
            expires_at_unix_ms,
            explanation,
            source_url,
        })
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub const fn chance(&self) -> RadarChance {
        self.chance
    }

    #[must_use]
    pub const fn observed_at_unix_ms(&self) -> i64 {
        self.observed_at_unix_ms
    }

    #[must_use]
    pub const fn expires_at_unix_ms(&self) -> i64 {
        self.expires_at_unix_ms
    }

    #[must_use]
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    #[must_use]
    pub fn source_url(&self) -> &str {
        &self.source_url
    }

    #[must_use]
    pub const fn is_active_at(&self, now_unix_ms: i64) -> bool {
        self.observed_at_unix_ms <= now_unix_ms && now_unix_ms < self.expires_at_unix_ms
    }
}

/// Strictly normalized latest global extra-reset announcement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarAnnouncement {
    source_id: String,
    announced_at_unix_ms: i64,
    text: String,
    source_url: String,
}

impl RadarAnnouncement {
    /// Validates one public source announcement.
    ///
    /// # Errors
    ///
    /// Rejects unsafe source links, invalid timestamps, IDs, or text.
    pub fn new(
        source_id: impl Into<String>,
        announced_at_unix_ms: i64,
        text: impl Into<String>,
        source_url: impl Into<String>,
    ) -> Result<Self, RadarContractError> {
        let source_id = source_id.into();
        validate_source_id(&source_id)?;
        let source_url = source_url.into();
        validate_source_url(&source_url, &source_id)?;
        if announced_at_unix_ms <= 0 {
            return Err(RadarContractError::InvalidTimeRange);
        }
        let text = text.into();
        validate_text(&text)?;
        Ok(Self {
            source_id,
            announced_at_unix_ms,
            text,
            source_url,
        })
    }

    #[must_use]
    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    #[must_use]
    pub const fn announced_at_unix_ms(&self) -> i64 {
        self.announced_at_unix_ms
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn source_url(&self) -> &str {
        &self.source_url
    }
}

/// One complete Radar result. Prediction and announcement are independent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RadarSnapshot {
    observation: Option<RadarObservation>,
    latest_announcement: Option<RadarAnnouncement>,
}

impl RadarSnapshot {
    #[must_use]
    pub const fn new(
        observation: Option<RadarObservation>,
        latest_announcement: Option<RadarAnnouncement>,
    ) -> Self {
        Self {
            observation,
            latest_announcement,
        }
    }

    #[must_use]
    pub fn observation(&self) -> Option<&RadarObservation> {
        self.observation.as_ref()
    }

    #[must_use]
    pub fn active_observation(&self, now_unix_ms: i64) -> Option<&RadarObservation> {
        self.observation
            .as_ref()
            .filter(|observation| observation.is_active_at(now_unix_ms))
    }

    #[must_use]
    pub fn latest_announcement(&self) -> Option<&RadarAnnouncement> {
        self.latest_announcement.as_ref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub enum RadarSourceErrorCode {
    Timeout,
    UpstreamUnavailable,
    ResponseTooLarge,
    InvalidJson,
    ContractViolation,
}

impl RadarSourceErrorCode {
    const STORAGE_KEYS: &'static [(Self, &'static str)] = &[
        (Self::Timeout, "timeout"),
        (Self::UpstreamUnavailable, "upstream_unavailable"),
        (Self::ResponseTooLarge, "response_too_large"),
        (Self::InvalidJson, "invalid_json"),
        (Self::ContractViolation, "contract_violation"),
    ];

    pub(crate) fn as_storage_key(self) -> &'static str {
        Self::STORAGE_KEYS
            .iter()
            .find_map(|(candidate, key)| (*candidate == self).then_some(*key))
            .expect("every radar source error code has one storage key")
    }

    pub(crate) fn from_storage_key(value: &str) -> Option<Self> {
        Self::STORAGE_KEYS
            .iter()
            .find_map(|(candidate, key)| (*key == value).then_some(*candidate))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicRadarPrediction {
    pub chance_basis_points: u16,
    pub display_chance: String,
    #[ts(type = "number")]
    pub observed_at_unix_ms: i64,
    #[ts(type = "number")]
    pub expires_at_unix_ms: i64,
    pub explanation: String,
    pub source_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicRadarAnnouncement {
    #[ts(type = "number")]
    pub announced_at_unix_ms: i64,
    pub text: String,
    pub source_url: String,
}

/// Secret-free Radar projection. Its health is independent from Codex usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../../ui/src/bindings/")]
pub struct PublicResetRadar {
    #[ts(type = "number | null")]
    pub last_attempt_at_unix_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub last_success_at_unix_ms: Option<i64>,
    pub consecutive_failures: u32,
    pub source_status: SourceStatus,
    pub public_error: Option<RadarSourceErrorCode>,
    pub prediction: Option<PublicRadarPrediction>,
    pub latest_announcement: Option<PublicRadarAnnouncement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadarCommitDisposition {
    pub new_announcement: bool,
}

/// Stable Radar failure with an internal cause that never crosses IPC.
#[derive(Debug, Clone, thiserror::Error)]
#[error("reset radar source failed: {code:?}")]
pub struct RadarSourceError {
    code: RadarSourceErrorCode,
    #[source]
    source: Option<Arc<dyn Error + Send + Sync>>,
}

impl RadarSourceError {
    #[must_use]
    pub const fn new(code: RadarSourceErrorCode) -> Self {
        Self { code, source: None }
    }

    #[must_use]
    pub fn with_source(
        code: RadarSourceErrorCode,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            code,
            source: Some(Arc::new(source)),
        }
    }

    #[must_use]
    pub const fn code(&self) -> RadarSourceErrorCode {
        self.code
    }
}

/// Object-safe anonymous source seam used by the refresh coordinator.
pub trait ResetRadarSource: Send + Sync + 'static {
    fn fetch_radar(
        &self,
        attempted_at_unix_ms: i64,
    ) -> Pin<Box<dyn Future<Output = Result<RadarSnapshot, RadarSourceError>> + Send + '_>>;
}

fn validate_source_id(value: &str) -> Result<(), RadarContractError> {
    if value.is_empty() || value.len() > 32 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(RadarContractError::InvalidSourceId);
    }
    Ok(())
}

fn validate_source_url(value: &str, source_id: &str) -> Result<(), RadarContractError> {
    let expected = format!("https://x.com/{SOURCE_ACCOUNT}/status/{source_id}");
    if value != expected {
        return Err(RadarContractError::UnsafeSourceUrl);
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), RadarContractError> {
    if value.trim().is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value
            .chars()
            .any(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
    {
        return Err(RadarContractError::InvalidExplanation);
    }
    Ok(())
}

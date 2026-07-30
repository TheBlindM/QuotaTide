use std::time::Duration;

use chrono::DateTime;
use futures_util::StreamExt as _;
use quotatide_core::{
    RadarAnnouncement, RadarObservation, RadarSnapshot, RadarSourceError, RadarSourceErrorCode,
    ResetRadarSource,
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

const RESET_RADAR_URL: &str = "https://codex-resets.com/api/resets";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Reusable anonymous, fixed-origin Reset Radar client.
#[derive(Clone)]
pub struct ResetRadarClient {
    client: Client,
}

impl ResetRadarClient {
    /// Builds the shared HTTP client with redirects disabled.
    ///
    /// # Errors
    ///
    /// Returns a stable source error if the native client cannot be configured.
    pub fn new() -> Result<Self, RadarSourceError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|source| {
                RadarSourceError::with_source(RadarSourceErrorCode::UpstreamUnavailable, source)
            })?;
        Ok(Self { client })
    }

    /// Fetches and strictly normalizes one public Radar response.
    ///
    /// # Errors
    ///
    /// Returns stable HTTP, timeout, size, JSON, or contract categories.
    pub async fn fetch(
        &self,
        attempted_at_unix_ms: i64,
    ) -> Result<RadarSnapshot, RadarSourceError> {
        let response = self
            .client
            .get(RESET_RADAR_URL)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(map_transport)?;
        if !response.status().is_success() {
            return Err(status_failure(response.status()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(RadarSourceError::new(
                RadarSourceErrorCode::ResponseTooLarge,
            ));
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(RadarSourceError::new(
                    RadarSourceErrorCode::ResponseTooLarge,
                ));
            }
            bytes.extend_from_slice(&chunk);
        }
        decode_reset_radar(&bytes, attempted_at_unix_ms)
    }
}

impl ResetRadarSource for ResetRadarClient {
    fn fetch_radar(
        &self,
        attempted_at_unix_ms: i64,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<RadarSnapshot, RadarSourceError>> + Send + '_>,
    > {
        Box::pin(self.fetch(attempted_at_unix_ms))
    }
}

#[derive(Deserialize)]
struct RadarDocument {
    watch: Option<RawWatch>,
    events: Vec<RawAnnouncement>,
}

#[derive(Deserialize)]
struct RawWatch {
    tweet_id: String,
    tweet_url: String,
    text: String,
    observed_at: String,
    expires_at: String,
    reset_chance_24h: f64,
    window_hours: f64,
}

#[derive(Deserialize)]
struct RawAnnouncement {
    tweet_id: String,
    tweet_url: String,
    text: String,
    announced_at: String,
}

/// Decodes the documented public contract without retaining the raw response.
///
/// # Errors
///
/// Returns `InvalidJson` for invalid JSON and `ContractViolation` for invalid
/// fields, probability, times, or source links.
pub fn decode_reset_radar(
    bytes: &[u8],
    now_unix_ms: i64,
) -> Result<RadarSnapshot, RadarSourceError> {
    let raw: serde_json::Value = serde_json::from_slice(bytes).map_err(|source| {
        RadarSourceError::with_source(RadarSourceErrorCode::InvalidJson, source)
    })?;
    let document: RadarDocument = serde_json::from_value(raw).map_err(|source| {
        RadarSourceError::with_source(RadarSourceErrorCode::ContractViolation, source)
    })?;
    let observation = document
        .watch
        .map(|watch| normalize_watch(watch, now_unix_ms))
        .transpose()?
        .flatten();
    let latest_announcement = document
        .events
        .into_iter()
        .map(normalize_announcement)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .max_by_key(RadarAnnouncement::announced_at_unix_ms);
    Ok(RadarSnapshot::new(observation, latest_announcement))
}

fn normalize_watch(
    watch: RawWatch,
    now_unix_ms: i64,
) -> Result<Option<RadarObservation>, RadarSourceError> {
    if !watch.window_hours.is_finite()
        || watch.window_hours <= 0.0
        || !watch.reset_chance_24h.is_finite()
        || !(0.0..=100.0).contains(&watch.reset_chance_24h)
    {
        return Err(contract_failure());
    }
    let chance_basis_points = (watch.reset_chance_24h * 100.0).round();
    if !(0.0..=10_000.0).contains(&chance_basis_points) {
        return Err(contract_failure());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let chance_basis_points = chance_basis_points as u16;
    let observed_at_unix_ms = parse_timestamp(&watch.observed_at)?;
    let expires_at_unix_ms = parse_timestamp(&watch.expires_at)?;
    let observation = RadarObservation::new(
        watch.tweet_id,
        chance_basis_points,
        observed_at_unix_ms,
        expires_at_unix_ms,
        watch.text,
        watch.tweet_url,
    )
    .map_err(|source| {
        RadarSourceError::with_source(RadarSourceErrorCode::ContractViolation, source)
    })?;
    if observed_at_unix_ms > now_unix_ms || !observation.is_active_at(now_unix_ms) {
        return Ok(None);
    }
    Ok(Some(observation))
}

fn normalize_announcement(
    announcement: RawAnnouncement,
) -> Result<RadarAnnouncement, RadarSourceError> {
    let announced_at_unix_ms = parse_timestamp(&announcement.announced_at)?;
    RadarAnnouncement::new(
        announcement.tweet_id,
        announced_at_unix_ms,
        announcement.text,
        announcement.tweet_url,
    )
    .map_err(|source| {
        RadarSourceError::with_source(RadarSourceErrorCode::ContractViolation, source)
    })
}

fn parse_timestamp(value: &str) -> Result<i64, RadarSourceError> {
    DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.timestamp_millis())
        .map_err(|source| {
            RadarSourceError::with_source(RadarSourceErrorCode::ContractViolation, source)
        })
}

fn map_transport(error: reqwest::Error) -> RadarSourceError {
    let code = if error.is_timeout() {
        RadarSourceErrorCode::Timeout
    } else {
        RadarSourceErrorCode::UpstreamUnavailable
    };
    RadarSourceError::with_source(code, error)
}

fn status_failure(_status: StatusCode) -> RadarSourceError {
    RadarSourceError::new(RadarSourceErrorCode::UpstreamUnavailable)
}

const fn contract_failure() -> RadarSourceError {
    RadarSourceError::new(RadarSourceErrorCode::ContractViolation)
}

use std::time::Duration;

use chrono::DateTime;
use futures_util::StreamExt as _;
use quotatide_core::{
    RadarAnnouncement, RadarObservation, RadarSnapshot, RadarSourceError, RadarSourceErrorCode,
    ResetRadarSource,
};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

const RESET_RADAR_URL: &str = "https://www.codexrunway.com/api/status.json";
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_DOCUMENT_AGE_MS: i64 = 6 * 60 * 60 * 1_000;
const MAX_FUTURE_CLOCK_SKEW_MS: i64 = 5 * 60 * 1_000;

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
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    #[serde(rename = "generatedAt")]
    generated_at: String,
    #[serde(rename = "lastSuccessfulCheckAt")]
    last_successful_check_at: Option<String>,
    monitor: RawMonitor,
    events: Vec<RawEvent>,
}

#[derive(Deserialize)]
struct RawMonitor {
    status: String,
    #[serde(rename = "errorCode")]
    error_code: Option<String>,
}

#[derive(Deserialize)]
struct RawEvent {
    kind: String,
    #[serde(rename = "announcedAt")]
    announced_at: String,
    #[serde(rename = "effectiveAt")]
    effective_at: Option<String>,
    source: RawSource,
    confidence: f64,
    rationale: String,
}

#[derive(Deserialize)]
struct RawSource {
    handle: String,
    #[serde(rename = "postId")]
    post_id: String,
    url: String,
}

/// Decodes the documented public contract without retaining the raw response.
///
/// # Errors
///
/// Returns `InvalidJson` for invalid JSON, `UpstreamUnavailable` when the
/// published monitor reports a failed refresh, and `ContractViolation` for
/// invalid fields, confidence, times, or source links.
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
    if document.schema_version != 1 {
        return Err(contract_failure());
    }
    let generated_at_unix_ms = parse_timestamp(&document.generated_at)?;
    if generated_at_unix_ms > now_unix_ms.saturating_add(MAX_FUTURE_CLOCK_SKEW_MS) {
        return Err(contract_failure());
    }
    if now_unix_ms.saturating_sub(generated_at_unix_ms) > MAX_DOCUMENT_AGE_MS {
        return Err(RadarSourceError::new(
            RadarSourceErrorCode::UpstreamUnavailable,
        ));
    }
    if let Some(last_successful_check_at) = &document.last_successful_check_at {
        parse_timestamp(last_successful_check_at)?;
    }
    if document.monitor.status != "ok" {
        return Err(RadarSourceError::new(
            RadarSourceErrorCode::UpstreamUnavailable,
        ));
    }
    if document.monitor.error_code.is_some() {
        return Err(contract_failure());
    }

    let mut observations = Vec::new();
    let mut announcements = Vec::new();
    for event in document.events {
        match event.kind.as_str() {
            "reset_scheduled" => {
                if let Some(observation) = normalize_prediction(event, now_unix_ms)? {
                    observations.push(observation);
                }
            }
            "reset_completed" => {
                if let Some(announcement) = normalize_announcement(event, now_unix_ms)? {
                    announcements.push(announcement);
                }
            }
            _ => {}
        }
    }
    let observation = observations
        .into_iter()
        .min_by_key(RadarObservation::expires_at_unix_ms);
    let latest_announcement = announcements
        .into_iter()
        .max_by_key(RadarAnnouncement::announced_at_unix_ms);
    Ok(RadarSnapshot::new(observation, latest_announcement))
}

fn normalize_prediction(
    event: RawEvent,
    now_unix_ms: i64,
) -> Result<Option<RadarObservation>, RadarSourceError> {
    validate_source(&event.source)?;
    if !event.confidence.is_finite() || !(0.0..=1.0).contains(&event.confidence) {
        return Err(contract_failure());
    }
    let chance_basis_points = (event.confidence * 10_000.0).round();
    if !(0.0..=10_000.0).contains(&chance_basis_points) {
        return Err(contract_failure());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let chance_basis_points = chance_basis_points as u16;
    let observed_at_unix_ms = parse_timestamp(&event.announced_at)?;
    let expires_at_unix_ms = event
        .effective_at
        .as_deref()
        .ok_or_else(contract_failure)
        .and_then(parse_timestamp)?;
    let observation = RadarObservation::new(
        event.source.post_id,
        chance_basis_points,
        observed_at_unix_ms,
        expires_at_unix_ms,
        event.rationale,
        event.source.url,
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
    event: RawEvent,
    now_unix_ms: i64,
) -> Result<Option<RadarAnnouncement>, RadarSourceError> {
    validate_source(&event.source)?;
    if !event.confidence.is_finite() || !(0.0..=1.0).contains(&event.confidence) {
        return Err(contract_failure());
    }
    let announced_at_unix_ms = parse_timestamp(&event.announced_at)?;
    if announced_at_unix_ms > now_unix_ms {
        return Ok(None);
    }
    RadarAnnouncement::new(
        event.source.post_id,
        announced_at_unix_ms,
        event.rationale,
        event.source.url,
    )
    .map_err(|source| {
        RadarSourceError::with_source(RadarSourceErrorCode::ContractViolation, source)
    })
    .map(Some)
}

fn validate_source(source: &RawSource) -> Result<(), RadarSourceError> {
    if source.handle != "thsottiaux" {
        return Err(contract_failure());
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::{RESET_RADAR_URL, RadarSourceErrorCode, decode_reset_radar};
    use chrono::DateTime;
    use serde_json::{Value, json};

    fn timestamp(value: &str) -> i64 {
        DateTime::parse_from_rfc3339(value)
            .expect("test timestamp")
            .timestamp_millis()
    }

    fn valid_document() -> Value {
        json!({
          "schemaVersion": 1,
          "generatedAt": "2026-08-03T12:00:00Z",
          "lastSuccessfulCheckAt": "2026-08-03T11:59:00Z",
          "monitor": { "status": "ok", "errorCode": null },
          "events": [
            {
              "kind": "reset_scheduled",
              "announcedAt": "2026-08-03T11:00:00Z",
              "effectiveAt": "2026-08-04T11:00:00Z",
              "source": {
                "handle": "thsottiaux",
                "postId": "1952345678901234567",
                "url": "https://x.com/thsottiaux/status/1952345678901234567"
              },
              "confidence": 0.74,
              "rationale": "Public post suggests an extra reset."
            }
          ]
        })
    }

    #[test]
    fn client_targets_the_current_codex_runway_feed() {
        assert_eq!(
            RESET_RADAR_URL,
            "https://www.codexrunway.com/api/status.json"
        );
    }

    #[test]
    fn current_feed_ignores_uncertain_and_banked_events() {
        let document = json!({
          "schemaVersion": 1,
          "generatedAt": "2026-08-05T02:27:02.764Z",
          "lastSuccessfulCheckAt": "2026-08-05T02:27:02.764Z",
          "monitor": { "status": "ok", "errorCode": null },
          "events": [
            {
              "kind": "uncertain",
              "announcedAt": "2026-08-05T01:31:42.000Z",
              "effectiveAt": null,
              "source": {
                "handle": "thsottiaux",
                "postId": "2084814573090296270",
                "url": "https://x.com/thsottiaux/status/2084814573090296270"
              },
              "confidence": 0.6,
              "rationale": "Not a clear reset signal."
            },
            {
              "kind": "reset_completed",
              "announcedAt": "2026-08-01T03:32:37.000Z",
              "effectiveAt": null,
              "source": {
                "handle": "thsottiaux",
                "postId": "2083395449814229287",
                "url": "https://x.com/thsottiaux/status/2083395449814229287"
              },
              "confidence": 0.95,
              "rationale": "Explicit Codex quota reset announcement."
            },
            {
              "kind": "banked_reset",
              "announcedAt": "2026-07-13T18:29:31.000Z",
              "effectiveAt": null,
              "source": {
                "handle": "thsottiaux",
                "postId": "2076735790567338203",
                "url": "https://x.com/thsottiaux/status/2076735790567338203"
              },
              "confidence": 0.9,
              "rationale": "A reset credit was banked."
            }
          ]
        });

        let snapshot = decode_reset_radar(
            &serde_json::to_vec(&document).expect("encode current feed"),
            timestamp("2026-08-05T02:30:00Z"),
        )
        .expect("current Codex Runway feed");

        assert!(snapshot.observation().is_none());
        assert_eq!(
            snapshot
                .latest_announcement()
                .expect("completed reset announcement")
                .source_id(),
            "2083395449814229287"
        );
    }

    #[test]
    fn documented_contract_normalizes_one_active_prediction() {
        let snapshot = decode_reset_radar(
            &serde_json::to_vec(&valid_document()).expect("encode fixture"),
            timestamp("2026-08-03T12:00:00Z"),
        )
        .expect("valid radar contract");
        let observation = snapshot.observation().expect("active prediction");

        assert_eq!(observation.chance().basis_points(), 7_400);
        assert_eq!(
            observation.expires_at_unix_ms(),
            timestamp("2026-08-04T11:00:00Z")
        );
        assert_eq!(observation.source_id(), "1952345678901234567");
    }

    #[test]
    fn schema_or_approved_source_changes_fail_closed() {
        for document in [
            {
                let mut value = valid_document();
                value["schemaVersion"] = json!(2);
                value
            },
            {
                let mut value = valid_document();
                value["events"][0]["source"]["handle"] = json!("someone_else");
                value
            },
        ] {
            let error = decode_reset_radar(
                &serde_json::to_vec(&document).expect("encode fixture"),
                timestamp("2026-08-03T12:00:00Z"),
            )
            .expect_err("changed contract must be rejected");
            assert_eq!(error.code(), RadarSourceErrorCode::ContractViolation);
        }
    }

    #[test]
    fn upstream_monitor_failure_is_not_presented_as_a_prediction() {
        let mut document = valid_document();
        document["monitor"]["status"] = json!("error");
        document["monitor"]["errorCode"] = json!("fetch_failed");

        let error = decode_reset_radar(
            &serde_json::to_vec(&document).expect("encode fixture"),
            timestamp("2026-08-03T12:00:00Z"),
        )
        .expect_err("failed monitor must stay unavailable");
        assert_eq!(error.code(), RadarSourceErrorCode::UpstreamUnavailable);
    }

    #[test]
    fn stale_or_implausibly_future_documents_never_look_fresh() {
        let stale = decode_reset_radar(
            &serde_json::to_vec(&valid_document()).expect("encode stale fixture"),
            timestamp("2026-08-03T18:00:01Z"),
        )
        .expect_err("stale published document");
        assert_eq!(stale.code(), RadarSourceErrorCode::UpstreamUnavailable);

        let mut future_document = valid_document();
        future_document["generatedAt"] = json!("2026-08-03T12:06:00Z");
        let future = decode_reset_radar(
            &serde_json::to_vec(&future_document).expect("encode future fixture"),
            timestamp("2026-08-03T12:00:00Z"),
        )
        .expect_err("future published document");
        assert_eq!(future.code(), RadarSourceErrorCode::ContractViolation);
    }
}

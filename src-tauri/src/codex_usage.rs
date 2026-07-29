use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::StreamExt as _;
use quotatide_core::{
    QuotaUnits, UsageRefreshSource, UsageSourceErrorCode, WeeklyUsageObservation,
};
use reqwest::{Client, StatusCode};
use secrecy::{ExposeSecret as _, SecretString};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::auth_file::read_auth_file;

const WHAM_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const WEEK_SECONDS: u32 = 604_800;
const MAX_RESPONSE_BYTES: usize = 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Internal adapter error. It contains no response body, token, or account identity.
#[derive(Debug, thiserror::Error)]
#[error("Codex usage source failed: {code:?}")]
pub struct CodexUsageError {
    code: UsageSourceErrorCode,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl CodexUsageError {
    #[must_use]
    pub const fn code(&self) -> UsageSourceErrorCode {
        self.code
    }
}

/// Reusable fixed-origin WHAM client.
#[derive(Clone)]
pub struct CodexUsageClient {
    client: Client,
}

struct RequestCredentials {
    access_token: SecretString,
    account_id: String,
    fingerprint: [u8; 32],
}

trait AuthMaterialSource: Clone + Send + Sync + 'static {
    fn read_current(&self) -> Result<RequestCredentials, UsageSourceErrorCode>;
}

trait UsageFetcher: Clone + Send + Sync + 'static {
    fn fetch<'a>(
        &'a self,
        credentials: &'a RequestCredentials,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceErrorCode>>
    + Send
    + 'a;
}

/// Mutable current path shared by settings and the read-only auth adapter.
#[derive(Clone)]
pub struct SelectedAuthFile {
    path: Arc<RwLock<Option<PathBuf>>>,
}

impl SelectedAuthFile {
    #[must_use]
    pub fn new(path: Option<PathBuf>) -> Self {
        Self {
            path: Arc::new(RwLock::new(path)),
        }
    }

    /// Replaces only the in-memory selected path after a successful settings commit.
    ///
    /// # Errors
    ///
    /// Returns an error if the in-memory path lock is unavailable.
    pub fn replace(&self, path: PathBuf) -> Result<(), UsageSourceErrorCode> {
        *self
            .path
            .write()
            .map_err(|_| UsageSourceErrorCode::AuthPathUnavailable)? = Some(path);
        Ok(())
    }
}

impl AuthMaterialSource for SelectedAuthFile {
    fn read_current(&self) -> Result<RequestCredentials, UsageSourceErrorCode> {
        let path = self
            .path
            .read()
            .map_err(|_| UsageSourceErrorCode::AuthPathUnavailable)?
            .clone()
            .ok_or(UsageSourceErrorCode::AuthPathUnavailable)?;
        credentials_from_file(&path)
    }
}

/// Production current-account collector. Every attempt reopens auth.json.
#[derive(Clone)]
pub struct CodexUsageCollector<A = SelectedAuthFile, F = CodexUsageClient> {
    auth: A,
    fetcher: F,
}

impl CodexUsageCollector {
    #[must_use]
    pub const fn new(auth: SelectedAuthFile, fetcher: CodexUsageClient) -> Self {
        Self { auth, fetcher }
    }
}

impl<A: AuthMaterialSource, F: UsageFetcher> UsageRefreshSource for CodexUsageCollector<A, F> {
    fn fetch(
        &self,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceErrorCode>> + Send
    {
        let auth = self.auth.clone();
        let fetcher = self.fetcher.clone();
        async move {
            let first = auth.read_current()?;
            match fetcher.fetch(&first, captured_at_unix_ms).await {
                Err(
                    error @ (UsageSourceErrorCode::AuthenticationStale
                    | UsageSourceErrorCode::PermissionDenied),
                ) => {
                    let refreshed = auth.read_current()?;
                    if refreshed.fingerprint == first.fingerprint {
                        return Err(error);
                    }
                    fetcher.fetch(&refreshed, captured_at_unix_ms).await
                }
                result => result,
            }
        }
    }
}

impl UsageFetcher for CodexUsageClient {
    #[allow(clippy::manual_async_fn)]
    fn fetch<'a>(
        &'a self,
        credentials: &'a RequestCredentials,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceErrorCode>>
    + Send
    + 'a {
        async move {
            self.fetch(
                &credentials.access_token,
                &credentials.account_id,
                captured_at_unix_ms,
            )
            .await
            .map_err(|error| error.code())
        }
    }
}

fn credentials_from_file(path: &Path) -> Result<RequestCredentials, UsageSourceErrorCode> {
    let material = read_auth_file(path).map_err(|_| UsageSourceErrorCode::AuthPathUnavailable)?;
    let mut hasher = Sha256::new();
    hasher.update(material.access_token().expose_secret().as_bytes());
    hasher.update([0]);
    hasher.update(material.account_id().as_bytes());
    Ok(RequestCredentials {
        access_token: SecretString::from(material.access_token().expose_secret().to_owned()),
        account_id: material.account_id().to_owned(),
        fingerprint: hasher.finalize().into(),
    })
}

impl CodexUsageClient {
    /// Builds the shared HTTP client.
    ///
    /// # Errors
    ///
    /// Returns an upstream adapter error if the client cannot be configured.
    pub fn new() -> Result<Self, CodexUsageError> {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|source| failure(UsageSourceErrorCode::UpstreamUnavailable, source))?;
        Ok(Self { client })
    }

    fn request(&self, access_token: &SecretString, account_id: &str) -> reqwest::RequestBuilder {
        self.client
            .get(WHAM_USAGE_URL)
            .bearer_auth(access_token.expose_secret())
            .header("chatgpt-account-id", account_id)
            .header("originator", "codex_cli_rs")
            .header(reqwest::header::ACCEPT, "application/json")
    }

    /// Fetches and strictly normalizes the current seven-day window.
    ///
    /// # Errors
    ///
    /// Returns a stable source category for HTTP, timeout, body-size, JSON, or
    /// response-contract failures.
    pub async fn fetch(
        &self,
        access_token: &SecretString,
        account_id: &str,
        captured_at_unix_ms: i64,
    ) -> Result<WeeklyUsageObservation, CodexUsageError> {
        let response = self
            .request(access_token, account_id)
            .send()
            .await
            .map_err(map_transport)?;
        let status = response.status();
        if !status.is_success() {
            return Err(status_failure(status));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE_BYTES as u64)
        {
            return Err(simple(UsageSourceErrorCode::ResponseTooLarge));
        }

        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(map_transport)?;
            if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
                return Err(simple(UsageSourceErrorCode::ResponseTooLarge));
            }
            bytes.extend_from_slice(&chunk);
        }
        normalize_usage(&bytes, captured_at_unix_ms)
    }
}

#[derive(Deserialize)]
struct UsageDocument {
    plan_type: Option<String>,
    rate_limit: Option<RateLimit>,
}

#[derive(Deserialize)]
struct RateLimit {
    allowed: Option<bool>,
    primary_window: Option<UsageWindow>,
    secondary_window: Option<UsageWindow>,
}

#[derive(Deserialize)]
struct UsageWindow {
    used_percent: Option<f64>,
    limit_window_seconds: Option<u32>,
    reset_after_seconds: Option<i64>,
    reset_at: Option<i64>,
}

pub(crate) fn normalize_usage(
    bytes: &[u8],
    captured_at_unix_ms: i64,
) -> Result<WeeklyUsageObservation, CodexUsageError> {
    let document: UsageDocument = serde_json::from_slice(bytes)
        .map_err(|source| failure(UsageSourceErrorCode::InvalidJson, source))?;
    let rate_limit = document
        .rate_limit
        .ok_or_else(|| simple(UsageSourceErrorCode::ContractViolation))?;
    let windows = [rate_limit.primary_window, rate_limit.secondary_window];
    let mut matching = windows
        .into_iter()
        .flatten()
        .filter(|window| window.limit_window_seconds == Some(WEEK_SECONDS));
    let weekly = matching
        .next()
        .ok_or_else(|| simple(UsageSourceErrorCode::WeeklyWindowUnavailable))?;
    if matching.next().is_some() {
        return Err(simple(UsageSourceErrorCode::ContractViolation));
    }
    let used_percent = weekly
        .used_percent
        .filter(|value| value.is_finite() && (0.0..=100.0).contains(value))
        .ok_or_else(|| simple(UsageSourceErrorCode::ContractViolation))?;
    let reset_after = weekly
        .reset_after_seconds
        .filter(|value| *value >= 0)
        .ok_or_else(|| simple(UsageSourceErrorCode::ContractViolation))?;
    let resets_at = weekly
        .reset_at
        .filter(|value| *value > 0 && chrono::DateTime::from_timestamp(*value, 0).is_some())
        .ok_or_else(|| simple(UsageSourceErrorCode::ContractViolation))?;
    let _ = reset_after;
    let micropoints = percent_to_micropoints(used_percent);
    let used = QuotaUnits::from_micropoints(micropoints)
        .ok_or_else(|| simple(UsageSourceErrorCode::ContractViolation))?;

    Ok(WeeklyUsageObservation {
        captured_at_unix_ms,
        used,
        window_seconds: WEEK_SECONDS,
        resets_at_unix_s: resets_at,
        plan_type: document.plan_type,
        allowed: rate_limit.allowed,
    })
}

#[allow(clippy::cast_possible_truncation)]
fn percent_to_micropoints(percent: f64) -> i64 {
    // The caller has already proven the finite value is in 0..=100, so the
    // rounded result is exactly within the QuotaUnits i64 domain.
    (percent * 1_000_000.0).round() as i64
}

fn status_failure(status: StatusCode) -> CodexUsageError {
    simple(match status {
        StatusCode::UNAUTHORIZED => UsageSourceErrorCode::AuthenticationStale,
        StatusCode::FORBIDDEN => UsageSourceErrorCode::PermissionDenied,
        StatusCode::TOO_MANY_REQUESTS => UsageSourceErrorCode::RateLimited,
        _ => UsageSourceErrorCode::UpstreamUnavailable,
    })
}

fn map_transport(source: reqwest::Error) -> CodexUsageError {
    let code = if source.is_timeout() {
        UsageSourceErrorCode::Timeout
    } else {
        UsageSourceErrorCode::UpstreamUnavailable
    };
    failure(code, source)
}

fn simple(code: UsageSourceErrorCode) -> CodexUsageError {
    CodexUsageError { code, source: None }
}

fn failure(
    code: UsageSourceErrorCode,
    source: impl std::error::Error + Send + Sync + 'static,
) -> CodexUsageError {
    CodexUsageError {
        code,
        source: Some(Box::new(source)),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use secrecy::SecretString;
    use sha2::{Digest as _, Sha256};

    use super::{
        AuthMaterialSource, CodexUsageClient, CodexUsageCollector, RequestCredentials,
        UsageFetcher, normalize_usage,
    };
    use quotatide_core::{
        QuotaUnits, UsageRefreshSource, UsageSourceErrorCode, WeeklyUsageObservation,
    };

    const VALID: &str = r#"{
      "plan_type":"plus",
      "rate_limit":{
        "allowed":true,
        "primary_window":{
          "used_percent":3,
          "limit_window_seconds":18000,
          "reset_after_seconds":60,
          "reset_at":1781000000
        },
        "secondary_window":{
          "used_percent":41.25,
          "limit_window_seconds":604800,
          "reset_after_seconds":300000,
          "reset_at":1781300000
        }
      }
    }"#;

    #[test]
    fn request_contract_is_fixed_to_wham_without_redirects() {
        let client = CodexUsageClient::new().expect("client");
        let token = SecretString::from("access-ticket17-canary");
        let request = client
            .request(&token, "account-ticket17-canary")
            .build()
            .expect("request");

        assert_eq!(
            request.url().as_str(),
            "https://chatgpt.com/backend-api/wham/usage"
        );
        assert_eq!(
            request.headers()["authorization"],
            "Bearer access-ticket17-canary"
        );
        assert_eq!(
            request.headers()["chatgpt-account-id"],
            "account-ticket17-canary"
        );
        assert_eq!(request.headers()["originator"], "codex_cli_rs");
        assert_eq!(request.headers()["accept"], "application/json");
    }

    #[test]
    fn selects_only_the_exact_current_seven_day_window() {
        let observation =
            normalize_usage(VALID.as_bytes(), 1_785_000_000_000).expect("valid weekly observation");

        assert_eq!(observation.used.micropoints(), 41_250_000);
        assert_eq!(observation.window_seconds, 604_800);
        assert_eq!(observation.resets_at_unix_s, 1_781_300_000);
        assert_eq!(observation.plan_type.as_deref(), Some("plus"));
        assert_eq!(observation.allowed, Some(true));
    }

    #[test]
    fn rejects_missing_short_ambiguous_and_invalid_windows() {
        let cases = [
            (
                r#"{"rate_limit":{"primary_window":{"used_percent":1,"limit_window_seconds":18000,"reset_after_seconds":1,"reset_at":1781300000}}}"#,
                UsageSourceErrorCode::WeeklyWindowUnavailable,
            ),
            (
                r#"{"rate_limit":{"primary_window":{"used_percent":1,"limit_window_seconds":604800,"reset_after_seconds":1,"reset_at":1781300000},"secondary_window":{"used_percent":2,"limit_window_seconds":604800,"reset_after_seconds":1,"reset_at":1781300000}}}"#,
                UsageSourceErrorCode::ContractViolation,
            ),
            (
                r#"{"rate_limit":{"secondary_window":{"limit_window_seconds":604800,"reset_after_seconds":1,"reset_at":1781300000}}}"#,
                UsageSourceErrorCode::ContractViolation,
            ),
            (
                r#"{"rate_limit":{"secondary_window":{"used_percent":101,"limit_window_seconds":604800,"reset_after_seconds":1,"reset_at":1781300000}}}"#,
                UsageSourceErrorCode::ContractViolation,
            ),
            (
                r#"{"rate_limit":{"secondary_window":{"used_percent":1,"limit_window_seconds":604800,"reset_after_seconds":-1,"reset_at":1781300000}}}"#,
                UsageSourceErrorCode::ContractViolation,
            ),
            (
                r#"{"rate_limit":{"secondary_window":{"used_percent":1,"limit_window_seconds":604800,"reset_after_seconds":1,"reset_at":9223372036854775807}}}"#,
                UsageSourceErrorCode::ContractViolation,
            ),
        ];

        for (payload, expected) in cases {
            let error = normalize_usage(payload.as_bytes(), 0).expect_err("invalid window");
            assert_eq!(error.code(), expected);
            assert!(!format!("{error:?}").contains(payload));
        }
    }

    #[derive(Clone)]
    struct FakeAuth {
        reads: Arc<AtomicUsize>,
        credentials: Arc<Mutex<VecDeque<RequestCredentials>>>,
    }

    impl AuthMaterialSource for FakeAuth {
        fn read_current(&self) -> Result<RequestCredentials, UsageSourceErrorCode> {
            self.reads.fetch_add(1, Ordering::SeqCst);
            let mut credentials = self.credentials.lock().expect("credential queue");
            if credentials.len() > 1 {
                Ok(credentials.pop_front().expect("queued credentials"))
            } else {
                credentials
                    .pop_front()
                    .ok_or(UsageSourceErrorCode::AuthPathUnavailable)
            }
        }
    }

    #[derive(Clone)]
    struct FakeFetcher {
        calls: Arc<AtomicUsize>,
        results: Arc<Mutex<VecDeque<Result<WeeklyUsageObservation, UsageSourceErrorCode>>>>,
    }

    impl UsageFetcher for FakeFetcher {
        #[allow(clippy::manual_async_fn)]
        fn fetch<'a>(
            &'a self,
            _credentials: &'a RequestCredentials,
            _captured_at_unix_ms: i64,
        ) -> impl std::future::Future<
            Output = Result<WeeklyUsageObservation, UsageSourceErrorCode>,
        > + Send
        + 'a {
            async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.results
                    .lock()
                    .expect("result queue")
                    .pop_front()
                    .expect("queued result")
            }
        }
    }

    #[tokio::test]
    async fn authentication_failure_retries_once_only_after_token_rotation() {
        let reads = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = CodexUsageCollector {
            auth: FakeAuth {
                reads: Arc::clone(&reads),
                credentials: Arc::new(Mutex::new(VecDeque::from([
                    credentials("token-a"),
                    credentials("token-b"),
                ]))),
            },
            fetcher: FakeFetcher {
                calls: Arc::clone(&calls),
                results: Arc::new(Mutex::new(VecDeque::from([
                    Err(UsageSourceErrorCode::AuthenticationStale),
                    Ok(test_observation()),
                ]))),
            },
        };

        let result = collector.fetch(1_785_000_000_000).await;

        assert!(result.is_ok());
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn unchanged_credentials_preserve_the_original_permission_failure() {
        let reads = Arc::new(AtomicUsize::new(0));
        let calls = Arc::new(AtomicUsize::new(0));
        let collector = CodexUsageCollector {
            auth: FakeAuth {
                reads: Arc::clone(&reads),
                credentials: Arc::new(Mutex::new(VecDeque::from([
                    credentials("token-a"),
                    credentials("token-a"),
                ]))),
            },
            fetcher: FakeFetcher {
                calls: Arc::clone(&calls),
                results: Arc::new(Mutex::new(VecDeque::from([Err(
                    UsageSourceErrorCode::PermissionDenied,
                )]))),
            },
        };

        let result = collector.fetch(1_785_000_000_000).await;

        assert_eq!(result, Err(UsageSourceErrorCode::PermissionDenied));
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    fn credentials(token: &str) -> RequestCredentials {
        let fingerprint = Sha256::digest(token.as_bytes()).into();
        RequestCredentials {
            access_token: SecretString::from(token.to_owned()),
            account_id: "account-ticket17".to_owned(),
            fingerprint,
        }
    }

    fn test_observation() -> WeeklyUsageObservation {
        WeeklyUsageObservation {
            captured_at_unix_ms: 1_785_000_000_000,
            used: QuotaUnits::from_micropoints(20_000_000).expect("valid quota"),
            window_seconds: 604_800,
            resets_at_unix_s: 1_786_000_000,
            plan_type: Some("plus".to_owned()),
            allowed: Some(true),
        }
    }
}

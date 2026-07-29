use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt as _;
use quotatide_core::{
    AccountSettingsStore, QuotaUnits, RefreshAccountBinding, UsageRefreshAttempt,
    UsageRefreshSource, UsageSourceError, UsageSourceErrorCode, WeeklyUsageObservation,
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

struct CurrentAuth {
    binding: RefreshAccountBinding,
    credentials: RequestCredentials,
}

struct AuthReadFailure {
    binding: Option<RefreshAccountBinding>,
    error: UsageSourceError,
}

trait AuthMaterialSource: Clone + Send + Sync + 'static {
    fn read_current(
        &self,
    ) -> impl std::future::Future<Output = Result<CurrentAuth, AuthReadFailure>> + Send;
}

trait UsageFetcher: Clone + Send + Sync + 'static {
    fn fetch<'a>(
        &'a self,
        credentials: &'a RequestCredentials,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceError>> + Send + 'a;
}

/// Read-only auth source backed by the versioned settings store.
#[derive(Clone)]
pub struct ConfiguredAuthFile {
    store: AccountSettingsStore,
}

impl ConfiguredAuthFile {
    #[must_use]
    pub const fn new(store: AccountSettingsStore) -> Self {
        Self { store }
    }
}

impl AuthMaterialSource for ConfiguredAuthFile {
    #[allow(clippy::manual_async_fn)]
    fn read_current(
        &self,
    ) -> impl std::future::Future<Output = Result<CurrentAuth, AuthReadFailure>> + Send {
        let store = self.store.clone();
        async move {
            let binding = store
                .configured_refresh_binding()
                .await
                .map_err(|error| AuthReadFailure {
                    binding: None,
                    error: UsageSourceError::with_source(
                        UsageSourceErrorCode::AuthPathUnavailable,
                        error,
                    ),
                })?
                .ok_or_else(|| AuthReadFailure {
                    binding: None,
                    error: UsageSourceError::new(UsageSourceErrorCode::AuthPathUnavailable),
                })?;
            let credentials = credentials_from_file(binding.canonical_path()).map_err(|error| {
                AuthReadFailure {
                    binding: Some(binding.clone()),
                    error,
                }
            })?;
            let binding = binding.with_account_id(credentials.account_id.clone());
            Ok(CurrentAuth {
                binding,
                credentials,
            })
        }
    }
}

/// Production current-account collector. Every attempt reopens auth.json.
#[derive(Clone)]
pub struct CodexUsageCollector<A = ConfiguredAuthFile, F = CodexUsageClient> {
    auth: A,
    fetcher: F,
}

impl CodexUsageCollector {
    #[must_use]
    pub const fn new(auth: ConfiguredAuthFile, fetcher: CodexUsageClient) -> Self {
        Self { auth, fetcher }
    }
}

impl<A: AuthMaterialSource, F: UsageFetcher> UsageRefreshSource for CodexUsageCollector<A, F> {
    fn fetch(
        &self,
        captured_at_unix_ms: i64,
    ) -> impl std::future::Future<Output = UsageRefreshAttempt> + Send {
        let auth = self.auth.clone();
        let fetcher = self.fetcher.clone();
        async move {
            let CurrentAuth {
                binding,
                credentials: first,
            } = match auth.read_current().await {
                Ok(current) => current,
                Err(failure) => {
                    return UsageRefreshAttempt::failure(failure.binding, failure.error);
                }
            };
            match fetcher.fetch(&first, captured_at_unix_ms).await {
                Ok(observation) => UsageRefreshAttempt::success(binding, observation),
                Err(error)
                    if matches!(
                        error.code(),
                        UsageSourceErrorCode::AuthenticationStale
                            | UsageSourceErrorCode::PermissionDenied
                    ) =>
                {
                    let CurrentAuth {
                        binding: refreshed_binding,
                        credentials: refreshed,
                    } = match auth.read_current().await {
                        Ok(current) => current,
                        Err(failure) => {
                            return UsageRefreshAttempt::failure(
                                failure.binding.or(Some(binding)),
                                UsageSourceError::with_source(
                                    UsageSourceErrorCode::AuthenticationStale,
                                    failure.error,
                                ),
                            );
                        }
                    };
                    if refreshed.fingerprint == first.fingerprint {
                        return UsageRefreshAttempt::failure(
                            Some(binding),
                            UsageSourceError::with_source(
                                UsageSourceErrorCode::AuthenticationStale,
                                error,
                            ),
                        );
                    }
                    match fetcher.fetch(&refreshed, captured_at_unix_ms).await {
                        Ok(observation) => {
                            UsageRefreshAttempt::success(refreshed_binding, observation)
                        }
                        Err(retry_error) => UsageRefreshAttempt::failure(
                            Some(refreshed_binding),
                            UsageSourceError::with_source(
                                UsageSourceErrorCode::AuthenticationStale,
                                retry_error,
                            ),
                        ),
                    }
                }
                Err(error) => UsageRefreshAttempt::failure(Some(binding), error),
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
    ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceError>> + Send + 'a
    {
        async move {
            self.fetch(
                &credentials.access_token,
                &credentials.account_id,
                captured_at_unix_ms,
            )
            .await
            .map_err(|error| UsageSourceError::with_source(error.code(), error))
        }
    }
}

fn credentials_from_file(path: &Path) -> Result<RequestCredentials, UsageSourceError> {
    let material = read_auth_file(path).map_err(|error| {
        UsageSourceError::with_source(UsageSourceErrorCode::AuthPathUnavailable, error)
    })?;
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
        AuthMaterialSource, AuthReadFailure, CodexUsageClient, CodexUsageCollector, CurrentAuth,
        RequestCredentials, UsageFetcher, normalize_usage,
    };
    use quotatide_core::{
        QuotaUnits, RefreshAccountBinding, UsageRefreshSource, UsageSourceError,
        UsageSourceErrorCode, WeeklyUsageObservation,
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
        #[allow(clippy::manual_async_fn)]
        fn read_current(
            &self,
        ) -> impl std::future::Future<Output = Result<CurrentAuth, AuthReadFailure>> + Send
        {
            let this = self.clone();
            async move {
                this.reads.fetch_add(1, Ordering::SeqCst);
                let mut credentials = this.credentials.lock().expect("credential queue");
                let current = credentials.pop_front().ok_or_else(|| AuthReadFailure {
                    binding: None,
                    error: UsageSourceError::new(UsageSourceErrorCode::AuthPathUnavailable),
                })?;
                let binding = RefreshAccountBinding::selected(
                    1,
                    std::path::PathBuf::from("/chosen/auth.json"),
                )
                .with_account_id(current.account_id.clone());
                Ok(CurrentAuth {
                    binding,
                    credentials: current,
                })
            }
        }
    }

    #[derive(Clone)]
    struct FakeFetcher {
        calls: Arc<AtomicUsize>,
        results: Arc<Mutex<VecDeque<Result<WeeklyUsageObservation, UsageSourceError>>>>,
    }

    impl UsageFetcher for FakeFetcher {
        #[allow(clippy::manual_async_fn)]
        fn fetch<'a>(
            &'a self,
            _credentials: &'a RequestCredentials,
            _captured_at_unix_ms: i64,
        ) -> impl std::future::Future<Output = Result<WeeklyUsageObservation, UsageSourceError>>
        + Send
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
                    Err(UsageSourceError::new(
                        UsageSourceErrorCode::AuthenticationStale,
                    )),
                    Ok(test_observation()),
                ]))),
            },
        };

        let result = collector.fetch(1_785_000_000_000).await.into_result();

        assert!(result.is_ok());
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn unchanged_credentials_normalize_permission_failure_as_stale_authentication() {
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
                results: Arc::new(Mutex::new(VecDeque::from([Err(UsageSourceError::new(
                    UsageSourceErrorCode::PermissionDenied,
                ))]))),
            },
        };

        let result = collector.fetch(1_785_000_000_000).await.into_result();

        assert_eq!(
            result.expect_err("stale authentication").code(),
            UsageSourceErrorCode::AuthenticationStale
        );
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_failed_rotation_retry_is_normalized_and_retains_its_source_chain() {
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
                    Err(UsageSourceError::new(
                        UsageSourceErrorCode::AuthenticationStale,
                    )),
                    Err(UsageSourceError::new(
                        UsageSourceErrorCode::PermissionDenied,
                    )),
                ]))),
            },
        };

        let error = collector
            .fetch(1_785_000_000_000)
            .await
            .into_result()
            .expect_err("retry remains stale");

        assert_eq!(error.code(), UsageSourceErrorCode::AuthenticationStale);
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(reads.load(Ordering::SeqCst), 2);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
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

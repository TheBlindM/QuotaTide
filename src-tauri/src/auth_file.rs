use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::SecretString;
use serde::Deserialize;

use quotatide_core::{
    AuthCandidateValidator, PublicError, PublicErrorCode, ValidatedAccountCandidate,
};

const MAX_AUTH_BYTES: u64 = 1024 * 1024;
const MAX_AUTH_BYTES_PUBLIC: u32 = 1024 * 1024;
const TRANSIENT_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Stable public category for an auth-file validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthFileErrorCode {
    NotFound,
    PermissionDenied,
    NotRegularFile,
    TooLarge,
    InvalidUtf8,
    InvalidJson,
    UnsupportedAuthMode,
    MissingAccessToken,
    MissingAccountId,
    InvalidAccountId,
}

/// Internal auth-file error. Sources are retained for diagnostics but never serialized.
#[derive(Debug)]
pub struct AuthFileError {
    code: AuthFileErrorCode,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl AuthFileError {
    #[must_use]
    pub const fn code(&self) -> AuthFileErrorCode {
        self.code
    }

    #[must_use]
    pub fn public(&self) -> PublicError {
        let (code, message_key) = match self.code {
            AuthFileErrorCode::NotFound => (PublicErrorCode::AuthNotFound, "auth.path.not_found"),
            AuthFileErrorCode::PermissionDenied => (
                PublicErrorCode::AuthPermissionDenied,
                "auth.path.permission_denied",
            ),
            AuthFileErrorCode::NotRegularFile => (
                PublicErrorCode::AuthNotRegularFile,
                "auth.path.not_regular_file",
            ),
            AuthFileErrorCode::TooLarge => {
                return PublicError::new(PublicErrorCode::AuthTooLarge, "auth.path.too_large")
                    .with_max_bytes(MAX_AUTH_BYTES_PUBLIC);
            }
            AuthFileErrorCode::InvalidUtf8 => {
                (PublicErrorCode::AuthInvalidUtf8, "auth.format.invalid_utf8")
            }
            AuthFileErrorCode::InvalidJson => {
                (PublicErrorCode::AuthInvalidJson, "auth.format.invalid_json")
            }
            AuthFileErrorCode::UnsupportedAuthMode => (
                PublicErrorCode::AuthUnsupportedMode,
                "auth.format.unsupported_mode",
            ),
            AuthFileErrorCode::MissingAccessToken => (
                PublicErrorCode::AuthMissingAccessToken,
                "auth.format.missing_access_token",
            ),
            AuthFileErrorCode::MissingAccountId => (
                PublicErrorCode::AuthMissingAccountId,
                "auth.format.missing_account_id",
            ),
            AuthFileErrorCode::InvalidAccountId => (
                PublicErrorCode::AuthInvalidAccountId,
                "auth.format.invalid_account_id",
            ),
        };
        PublicError::new(code, message_key)
    }
}

impl std::fmt::Display for AuthFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.public().message_key)
    }
}

impl std::error::Error for AuthFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Production validator adapter used by the core application facade.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthFileReader;

impl AuthCandidateValidator for AuthFileReader {
    type Error = AuthFileError;

    fn validate(&self, path: &Path) -> Result<ValidatedAccountCandidate, Self::Error> {
        let material = read_auth_file(path)?;
        let canonical_path = material
            .canonical_path()
            .to_str()
            .ok_or_else(|| error(AuthFileErrorCode::NotRegularFile))?
            .to_owned();
        Ok(ValidatedAccountCandidate::new(
            canonical_path,
            material.account_id().to_owned(),
        ))
    }

    fn public_error(error: &Self::Error) -> PublicError {
        error.public()
    }
}

/// Validated in-memory auth material. This type cannot be serialized or debug printed.
pub struct ValidatedAuth {
    canonical_path: PathBuf,
    access_token: SecretString,
    account_id: String,
}

impl ValidatedAuth {
    #[must_use]
    pub fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    #[must_use]
    pub const fn access_token(&self) -> &SecretString {
        &self.access_token
    }

    #[must_use]
    pub fn account_id(&self) -> &str {
        &self.account_id
    }
}

/// Reopens and strictly validates Codex-managed auth material without modifying it.
///
/// # Errors
///
/// Returns a stable [`AuthFileErrorCode`] for path, size, encoding, JSON, mode,
/// token, or account-identity failures. The error never contains source text.
pub fn read_auth_file(path: &Path) -> Result<ValidatedAuth, AuthFileError> {
    match read_auth_file_once(path) {
        Err(error)
            if matches!(
                error.code(),
                AuthFileErrorCode::NotFound | AuthFileErrorCode::InvalidJson
            ) =>
        {
            std::thread::sleep(TRANSIENT_RETRY_DELAY);
            read_auth_file_once(path)
        }
        result => result,
    }
}

fn read_auth_file_once(path: &Path) -> Result<ValidatedAuth, AuthFileError> {
    let canonical_path = fs::canonicalize(path).map_err(map_io_error)?;
    let metadata = fs::metadata(&canonical_path).map_err(map_io_error)?;
    if !metadata.is_file() {
        return Err(error(AuthFileErrorCode::NotRegularFile));
    }
    if metadata.len() > MAX_AUTH_BYTES {
        return Err(error(AuthFileErrorCode::TooLarge));
    }

    let mut file = File::open(&canonical_path).map_err(map_io_error)?;
    let mut bytes =
        Vec::with_capacity(usize::try_from(metadata.len().min(MAX_AUTH_BYTES)).unwrap_or_default());
    file.by_ref()
        .take(MAX_AUTH_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(map_io_error)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AUTH_BYTES {
        return Err(error(AuthFileErrorCode::TooLarge));
    }
    let json = std::str::from_utf8(&bytes)
        .map_err(|source| error_with_source(AuthFileErrorCode::InvalidUtf8, source))?;
    let document: AuthDocument = serde_json::from_str(json)
        .map_err(|source| error_with_source(AuthFileErrorCode::InvalidJson, source))?;
    if document.auth_mode.as_deref() != Some("chatgpt") {
        return Err(error(AuthFileErrorCode::UnsupportedAuthMode));
    }
    let tokens = document
        .tokens
        .ok_or_else(|| error(AuthFileErrorCode::MissingAccessToken))?;
    let access_token = required_canonical(tokens.access_token)
        .ok_or_else(|| error(AuthFileErrorCode::MissingAccessToken))?;
    let account_id = if let Some(candidate) = tokens.account_id {
        validate_account_id(candidate)?
    } else {
        let id_token = tokens
            .id_token
            .ok_or_else(|| error(AuthFileErrorCode::MissingAccountId))?;
        validate_account_id(account_id_from_jwt(&id_token)?)?
    };

    Ok(ValidatedAuth {
        canonical_path,
        access_token: SecretString::from(access_token),
        account_id,
    })
}

#[derive(Deserialize)]
struct AuthDocument {
    auth_mode: Option<String>,
    tokens: Option<AuthTokens>,
}

#[derive(Deserialize)]
struct AuthTokens {
    access_token: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
}

#[derive(Deserialize)]
struct JwtClaims {
    #[serde(rename = "https://api.openai.com/auth")]
    openai_auth: Option<OpenAiAuthClaims>,
}

#[derive(Deserialize)]
struct OpenAiAuthClaims {
    chatgpt_account_id: Option<String>,
}

fn account_id_from_jwt(id_token: &str) -> Result<String, AuthFileError> {
    let payload = id_token
        .split('.')
        .nth(1)
        .ok_or_else(|| error(AuthFileErrorCode::MissingAccountId))?;
    let decoded = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|source| error_with_source(AuthFileErrorCode::MissingAccountId, source))?;
    let claims: JwtClaims = serde_json::from_slice(&decoded)
        .map_err(|source| error_with_source(AuthFileErrorCode::MissingAccountId, source))?;
    claims
        .openai_auth
        .and_then(|auth| auth.chatgpt_account_id)
        .ok_or_else(|| error(AuthFileErrorCode::MissingAccountId))
}

fn validate_account_id(candidate: String) -> Result<String, AuthFileError> {
    let canonical = required_canonical(Some(candidate))
        .ok_or_else(|| error(AuthFileErrorCode::InvalidAccountId))?;
    if canonical.len() > 256
        || !canonical.is_ascii()
        || canonical
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return Err(error(AuthFileErrorCode::InvalidAccountId));
    }
    Ok(canonical)
}

fn required_canonical(candidate: Option<String>) -> Option<String> {
    candidate.filter(|value| !value.is_empty() && value.trim() == value)
}

fn map_io_error(source: std::io::Error) -> AuthFileError {
    let code = match source.kind() {
        std::io::ErrorKind::NotFound => AuthFileErrorCode::NotFound,
        std::io::ErrorKind::PermissionDenied => AuthFileErrorCode::PermissionDenied,
        _ => AuthFileErrorCode::NotRegularFile,
    };
    error_with_source(code, source)
}

const fn error(code: AuthFileErrorCode) -> AuthFileError {
    AuthFileError { code, source: None }
}

fn error_with_source(
    code: AuthFileErrorCode,
    source: impl std::error::Error + Send + Sync + 'static,
) -> AuthFileError {
    AuthFileError {
        code,
        source: Some(Box::new(source)),
    }
}

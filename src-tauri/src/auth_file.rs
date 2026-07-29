use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use secrecy::SecretString;
use serde::{Deserialize, Serialize};

const MAX_AUTH_BYTES: u64 = 1024 * 1024;
const TRANSIENT_RETRY_DELAY: Duration = Duration::from_millis(250);

/// Stable public category for an auth-file validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
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

/// Secret-free error payload suitable for IPC.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAuthFileError {
    pub code: AuthFileErrorCode,
    pub message_key: &'static str,
}

/// Internal auth-file error. It deliberately contains no path or parser source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthFileError {
    code: AuthFileErrorCode,
}

impl AuthFileError {
    #[must_use]
    pub const fn code(self) -> AuthFileErrorCode {
        self.code
    }

    #[must_use]
    pub const fn public(self) -> PublicAuthFileError {
        PublicAuthFileError {
            code: self.code,
            message_key: match self.code {
                AuthFileErrorCode::NotFound => "auth.path.not_found",
                AuthFileErrorCode::PermissionDenied => "auth.path.permission_denied",
                AuthFileErrorCode::NotRegularFile => "auth.path.not_regular_file",
                AuthFileErrorCode::TooLarge => "auth.path.too_large",
                AuthFileErrorCode::InvalidUtf8 => "auth.format.invalid_utf8",
                AuthFileErrorCode::InvalidJson => "auth.format.invalid_json",
                AuthFileErrorCode::UnsupportedAuthMode => "auth.format.unsupported_mode",
                AuthFileErrorCode::MissingAccessToken => "auth.format.missing_access_token",
                AuthFileErrorCode::MissingAccountId => "auth.format.missing_account_id",
                AuthFileErrorCode::InvalidAccountId => "auth.format.invalid_account_id",
            },
        }
    }
}

impl std::fmt::Display for AuthFileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.public().message_key)
    }
}

impl std::error::Error for AuthFileError {}

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
    let canonical_path = fs::canonicalize(path).map_err(|error| map_io_error(&error))?;
    let metadata = fs::metadata(&canonical_path).map_err(|error| map_io_error(&error))?;
    if !metadata.is_file() {
        return Err(error(AuthFileErrorCode::NotRegularFile));
    }
    if metadata.len() > MAX_AUTH_BYTES {
        return Err(error(AuthFileErrorCode::TooLarge));
    }

    let mut file = File::open(&canonical_path).map_err(|error| map_io_error(&error))?;
    let mut bytes =
        Vec::with_capacity(usize::try_from(metadata.len().min(MAX_AUTH_BYTES)).unwrap_or_default());
    file.by_ref()
        .take(MAX_AUTH_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| map_io_error(&error))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > MAX_AUTH_BYTES {
        return Err(error(AuthFileErrorCode::TooLarge));
    }
    let json = std::str::from_utf8(&bytes).map_err(|_| error(AuthFileErrorCode::InvalidUtf8))?;
    let document: AuthDocument =
        serde_json::from_str(json).map_err(|_| error(AuthFileErrorCode::InvalidJson))?;
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
        .map_err(|_| error(AuthFileErrorCode::MissingAccountId))?;
    let claims: JwtClaims =
        serde_json::from_slice(&decoded).map_err(|_| error(AuthFileErrorCode::MissingAccountId))?;
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

fn map_io_error(error_value: &std::io::Error) -> AuthFileError {
    error(match error_value.kind() {
        std::io::ErrorKind::NotFound => AuthFileErrorCode::NotFound,
        std::io::ErrorKind::PermissionDenied => AuthFileErrorCode::PermissionDenied,
        _ => AuthFileErrorCode::NotRegularFile,
    })
}

const fn error(code: AuthFileErrorCode) -> AuthFileError {
    AuthFileError { code }
}

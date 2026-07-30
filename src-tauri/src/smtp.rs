//! TLS-only SMTP adapter.

use std::sync::Arc;
use std::time::Duration;

use lettre::message::Mailbox;
use lettre::message::header::{ContentType, MessageId};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport as _, Message, Tokio1Executor};
use quotatide_core::{MailTransport, SafeMail, SmtpConnection, SmtpTlsMode};
use secrecy::{ExposeSecret, SecretString};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum SmtpTransportError {
    #[error("SMTP configuration is invalid")]
    InvalidConfiguration,
    #[error("SMTP delivery timed out")]
    Timeout,
    #[error("SMTP delivery failed")]
    Delivery { transient: bool },
}

#[derive(Clone, Default)]
pub struct LettreMailTransport {
    cached: Arc<Mutex<Option<CachedTransport>>>,
}

struct CachedTransport {
    key: [u8; 32],
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl MailTransport for LettreMailTransport {
    type Error = SmtpTransportError;

    async fn send(
        &self,
        connection: SmtpConnection,
        password: SecretString,
        mail: SafeMail,
    ) -> Result<(), Self::Error> {
        let message = build_message(&connection, &mail)?;
        let key = transport_key(&connection, &password);
        let transport = {
            let mut cached = self.cached.lock().await;
            if cached.as_ref().is_none_or(|current| current.key != key) {
                *cached = Some(CachedTransport {
                    key,
                    transport: build_transport(&connection, &password)?,
                });
            }
            cached
                .as_ref()
                .map(|current| current.transport.clone())
                .ok_or(SmtpTransportError::InvalidConfiguration)?
        };
        match tokio::time::timeout(Duration::from_secs(30), transport.send(message)).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(error)) => Err(SmtpTransportError::Delivery {
                transient: error.is_transient()
                    || error.is_timeout()
                    || error.is_transport_shutdown()
                    || (!error.is_permanent() && !error.is_tls() && !error.is_client()),
            }),
            Err(_) => Err(SmtpTransportError::Timeout),
        }
    }

    fn is_transient(error: &Self::Error) -> bool {
        matches!(
            error,
            SmtpTransportError::Timeout | SmtpTransportError::Delivery { transient: true }
        )
    }
}

fn build_transport(
    connection: &SmtpConnection,
    password: &SecretString,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, SmtpTransportError> {
    let builder = match connection.tls_mode {
        SmtpTlsMode::Tls => AsyncSmtpTransport::<Tokio1Executor>::relay(&connection.host),
        SmtpTlsMode::Starttls => {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&connection.host)
        }
    }
    .map_err(|_| SmtpTransportError::InvalidConfiguration)?;
    Ok(builder
        .port(connection.port)
        .credentials(Credentials::new(
            connection.username.clone(),
            password.expose_secret().to_owned(),
        ))
        .timeout(Some(Duration::from_secs(20)))
        .build())
}

fn build_message(
    connection: &SmtpConnection,
    mail: &SafeMail,
) -> Result<Message, SmtpTransportError> {
    let from_email = connection
        .from_address
        .parse()
        .map_err(|_| SmtpTransportError::InvalidConfiguration)?;
    let from = Mailbox::new(
        (!connection.from_name.is_empty()).then(|| connection.from_name.clone()),
        from_email,
    );
    let recipient = mail
        .recipient
        .parse()
        .map_err(|_| SmtpTransportError::InvalidConfiguration)?;
    let message_id = format!(
        "<{}@quotatide.local>",
        hex_digest(mail.delivery_key.as_bytes())
    );
    Message::builder()
        .from(from)
        .to(recipient)
        .subject(&mail.subject)
        .header(MessageId::from(message_id))
        .header(ContentType::TEXT_PLAIN)
        .body(mail.body.clone())
        .map_err(|_| SmtpTransportError::InvalidConfiguration)
}

fn transport_key(connection: &SmtpConnection, password: &SecretString) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(connection.host.as_bytes());
    hasher.update(connection.port.to_be_bytes());
    hasher.update(match connection.tls_mode {
        SmtpTlsMode::Tls => b"tls".as_slice(),
        SmtpTlsMode::Starttls => b"starttls".as_slice(),
    });
    hasher.update(connection.username.as_bytes());
    hasher.update(password.expose_secret().as_bytes());
    hasher.finalize().into()
}

fn hex_digest(value: &[u8]) -> String {
    Sha256::digest(value)
        .iter()
        .take(16)
        .fold(String::new(), |mut output, byte| {
            use std::fmt::Write as _;
            let _ = write!(output, "{byte:02x}");
            output
        })
}

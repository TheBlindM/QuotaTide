//! Narrow native notification boundary for macOS and Windows.
//!
//! The main workspace forbids unsafe Rust. Objective-C delegate construction
//! inherently requires unsafe message sends, so those two audited operations
//! live here instead of weakening the business core or desktop shell.

#![deny(unsafe_op_in_unsafe_fn)]

use std::sync::Arc;

use sha2::{Digest, Sha256};

pub type ActivationHandler = Arc<dyn Fn(NativeAlertTarget) + Send + Sync + 'static>;
pub type DeliveryFailureHandler = Arc<dyn Fn(String) + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeAlertTarget {
    Today,
    Radar,
    Source,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeNotification {
    pub delivery_key: String,
    pub title: String,
    pub body: String,
    pub target: NativeAlertTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativePermissionStatus {
    Unknown,
    Granted,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NativeNotificationError {
    #[error("notification authorization is unavailable")]
    Authorization,
    #[error("notification delivery failed")]
    Delivery,
    #[error("notification callback timed out")]
    Timeout,
    #[error("notification platform is unsupported")]
    Unsupported,
}

impl NativeNotificationError {
    #[must_use]
    pub const fn is_transient(self) -> bool {
        matches!(self, Self::Timeout)
    }
}

#[derive(Clone)]
pub struct NativeNotifier {
    #[cfg(target_os = "windows")]
    app_id: String,
    #[cfg(target_os = "windows")]
    activation: ActivationHandler,
    #[cfg(target_os = "windows")]
    delivery_failure: DeliveryFailureHandler,
    #[cfg(target_os = "windows")]
    pending_toasts: windows::PendingToasts,
    #[cfg(target_os = "macos")]
    _delegate: objc2::rc::Retained<macos::NotificationDelegate>,
}

impl NativeNotifier {
    /// Builds the platform notification adapter and installs its activation callback.
    ///
    /// # Errors
    ///
    /// Returns [`NativeNotificationError`] when the current platform cannot initialize
    /// native notifications.
    #[allow(clippy::needless_pass_by_value)] // Ownership is retained on Windows only.
    pub fn new(
        app_id: impl Into<String>,
        activation: ActivationHandler,
        delivery_failure: DeliveryFailureHandler,
    ) -> Result<Self, NativeNotificationError> {
        let app_id = app_id.into();
        #[cfg(target_os = "macos")]
        let delegate = macos::install_delegate(activation.clone());
        #[cfg(not(target_os = "windows"))]
        let _ = app_id;
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let _ = activation;
        #[cfg(not(target_os = "windows"))]
        let _ = delivery_failure;

        Ok(Self {
            #[cfg(target_os = "windows")]
            app_id,
            #[cfg(target_os = "windows")]
            activation,
            #[cfg(target_os = "windows")]
            delivery_failure,
            #[cfg(target_os = "windows")]
            pending_toasts: windows::PendingToasts::default(),
            #[cfg(target_os = "macos")]
            _delegate: delegate,
        })
    }

    /// Reads the operating system's current notification permission state.
    ///
    /// # Errors
    ///
    /// Returns [`NativeNotificationError`] when the native authorization API fails.
    pub fn permission_state(&self) -> Result<NativePermissionStatus, NativeNotificationError> {
        #[cfg(target_os = "macos")]
        {
            macos::permission_state()
        }
        #[cfg(target_os = "windows")]
        {
            windows::permission_state(&self.app_id)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(NativeNotificationError::Unsupported)
        }
    }

    /// Requests notification permission where the platform supports a prompt.
    ///
    /// # Errors
    ///
    /// Returns [`NativeNotificationError`] when the authorization request fails.
    pub fn request_permission(&self) -> Result<NativePermissionStatus, NativeNotificationError> {
        #[cfg(target_os = "macos")]
        {
            macos::request_permission()
        }
        #[cfg(target_os = "windows")]
        {
            windows::permission_state(&self.app_id)
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Err(NativeNotificationError::Unsupported)
        }
    }

    /// Sends one native notification using a stable delivery identity.
    ///
    /// # Errors
    ///
    /// Returns [`NativeNotificationError`] when the platform rejects or cannot deliver it.
    pub fn notify(&self, notification: &NativeNotification) -> Result<(), NativeNotificationError> {
        let identifier = stable_identifier(notification);
        #[cfg(target_os = "macos")]
        {
            macos::notify(notification, &identifier)
        }
        #[cfg(target_os = "windows")]
        {
            windows::notify(
                &self.app_id,
                &self.activation,
                &self.delivery_failure,
                &self.pending_toasts,
                notification.clone(),
                &identifier,
            )
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = notification;
            let _ = identifier;
            Err(NativeNotificationError::Unsupported)
        }
    }
}

fn stable_identifier(notification: &NativeNotification) -> String {
    let digest = Sha256::digest(notification.delivery_key.as_bytes());
    let mut short_hash = String::with_capacity(24);
    for byte in &digest[..12] {
        use std::fmt::Write as _;
        let _ = write!(short_hash, "{byte:02x}");
    }
    format!("quotatide-{short_hash}-{}", target_key(notification.target))
}

const fn target_key(target: NativeAlertTarget) -> &'static str {
    match target {
        NativeAlertTarget::Today => "today",
        NativeAlertTarget::Radar => "radar",
        NativeAlertTarget::Source => "source",
    }
}

#[cfg(any(target_os = "macos", test))]
fn target_from_identifier(identifier: &str) -> Option<NativeAlertTarget> {
    if identifier.ends_with("-today") {
        Some(NativeAlertTarget::Today)
    } else if identifier.ends_with("-radar") {
        Some(NativeAlertTarget::Radar)
    } else if identifier.ends_with("-source") {
        Some(NativeAlertTarget::Source)
    } else {
        None
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::fmt;
    use std::ptr::NonNull;
    use std::sync::mpsc;
    use std::time::Duration;

    use block2::{DynBlock, RcBlock};
    use objc2::rc::Retained;
    use objc2::runtime::{Bool, ProtocolObject};
    use objc2::{AnyThread, DefinedClass, define_class, msg_send};
    use objc2_foundation::{NSArray, NSError, NSObject, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
        UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNNotificationSettings, UNNotificationSound,
        UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };

    use super::{
        ActivationHandler, NativeNotification, NativeNotificationError, NativePermissionStatus,
        target_from_identifier,
    };

    const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

    pub(super) struct NotificationDelegateIvars {
        activation: ActivationHandler,
    }

    impl fmt::Debug for NotificationDelegateIvars {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("NotificationDelegateIvars")
        }
    }

    define_class!(
        // SAFETY: NSObject has no subclassing requirements. The retained
        // activation closure is Send + Sync and the delegate has no Drop impl.
        #[unsafe(super = NSObject)]
        #[thread_kind = AnyThread]
        #[ivars = NotificationDelegateIvars]
        pub(super) struct NotificationDelegate;

        // SAFETY: NSObjectProtocol has no additional invariants.
        unsafe impl NSObjectProtocol for NotificationDelegate {}

        // SAFETY: Method signatures match UNUserNotificationCenterDelegate.
        unsafe impl UNUserNotificationCenterDelegate for NotificationDelegate {
            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(
                &self,
                _center: &UNUserNotificationCenter,
                _notification: &UNNotification,
                completion: &DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
            ) {
                completion.call((UNNotificationPresentationOptions::Banner
                    | UNNotificationPresentationOptions::List
                    | UNNotificationPresentationOptions::Sound,));
            }

            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(
                &self,
                _center: &UNUserNotificationCenter,
                response: &UNNotificationResponse,
                completion: &DynBlock<dyn Fn()>,
            ) {
                let identifier = response.notification().request().identifier().to_string();
                if let Some(target) = target_from_identifier(&identifier) {
                    (self.ivars().activation)(target);
                }
                completion.call(());
            }
        }
    );

    impl NotificationDelegate {
        #[allow(unsafe_code)]
        fn new(activation: ActivationHandler) -> Retained<Self> {
            let this = Self::alloc().set_ivars(NotificationDelegateIvars { activation });
            // SAFETY: NSObject's `init` signature is correct and the ivars were
            // initialized exactly once before the superclass initializer.
            unsafe { msg_send![super(this), init] }
        }
    }

    pub(super) fn install_delegate(
        activation: ActivationHandler,
    ) -> Retained<NotificationDelegate> {
        let delegate = NotificationDelegate::new(activation);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        delegate
    }

    #[allow(unsafe_code)]
    pub(super) fn permission_state() -> Result<NativePermissionStatus, NativeNotificationError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let completion = RcBlock::new(move |settings: NonNull<UNNotificationSettings>| {
            // SAFETY: UserNotifications supplies a non-null settings object
            // valid for the duration of this callback.
            let status = unsafe { settings.as_ref() }.authorizationStatus();
            let _ = sender.send(status);
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .getNotificationSettingsWithCompletionHandler(&completion);
        receiver
            .recv_timeout(CALLBACK_TIMEOUT)
            .map(map_authorization_status)
            .map_err(|_| NativeNotificationError::Timeout)
    }

    pub(super) fn request_permission() -> Result<NativePermissionStatus, NativeNotificationError> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let completion = RcBlock::new(move |granted: Bool, error: *mut NSError| {
            let _ = sender.send((granted.as_bool(), error.is_null()));
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .requestAuthorizationWithOptions_completionHandler(
                UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
                &completion,
            );
        let (granted, no_error) = receiver
            .recv_timeout(CALLBACK_TIMEOUT)
            .map_err(|_| NativeNotificationError::Timeout)?;
        if !no_error {
            return Err(NativeNotificationError::Authorization);
        }
        Ok(if granted {
            NativePermissionStatus::Granted
        } else {
            NativePermissionStatus::Denied
        })
    }

    pub(super) fn notify(
        notification: &NativeNotification,
        identifier: &str,
    ) -> Result<(), NativeNotificationError> {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(&notification.title));
        content.setBody(&NSString::from_str(&notification.body));
        content.setSound(Some(&UNNotificationSound::defaultSound()));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(identifier),
            &content,
            None,
        );
        let identifier = NSString::from_str(identifier);
        let identifiers = NSArray::from_slice(&[&*identifier]);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        // Re-delivery after a crash replaces the already visible reminder
        // instead of stacking a second user-facing notification.
        center.removeDeliveredNotificationsWithIdentifiers(&identifiers);
        let (sender, receiver) = mpsc::sync_channel(1);
        let completion = RcBlock::new(move |error: *mut NSError| {
            let _ = sender.send(error.is_null());
        });
        center.addNotificationRequest_withCompletionHandler(&request, Some(&completion));
        match receiver.recv_timeout(CALLBACK_TIMEOUT) {
            Ok(true) => Ok(()),
            Ok(false) => Err(NativeNotificationError::Delivery),
            Err(_) => Err(NativeNotificationError::Timeout),
        }
    }

    fn map_authorization_status(status: UNAuthorizationStatus) -> NativePermissionStatus {
        if status == UNAuthorizationStatus::Authorized
            || status == UNAuthorizationStatus::Provisional
            || status == UNAuthorizationStatus::Ephemeral
        {
            NativePermissionStatus::Granted
        } else if status == UNAuthorizationStatus::Denied {
            NativePermissionStatus::Denied
        } else {
            NativePermissionStatus::Unknown
        }
    }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicU8, Ordering};
    use std::sync::{Arc, Mutex};

    use ::windows::Data::Xml::Dom::XmlDocument;
    use ::windows::Foundation::TypedEventHandler;
    use ::windows::UI::Notifications::{
        NotificationSetting, ToastFailedEventArgs, ToastNotification, ToastNotificationManager,
    };
    use ::windows::core::{HSTRING, IInspectable};

    use super::{
        ActivationHandler, DeliveryFailureHandler, NativeNotification, NativeNotificationError,
        NativePermissionStatus,
    };

    const DELIVERY_PENDING: u8 = 0;
    const DELIVERY_SUBMITTED: u8 = 1;
    const DELIVERY_FAILED_EARLY: u8 = 2;
    const MAX_PENDING_TOASTS: usize = 128;

    #[derive(Clone, Default)]
    pub(super) struct PendingToasts {
        inner: Arc<Mutex<VecDeque<(String, ToastNotification)>>>,
    }

    impl PendingToasts {
        fn insert(&self, identifier: String, toast: ToastNotification) {
            if let Ok(mut pending) = self.inner.lock() {
                pending.retain(|(current, _)| current != &identifier);
                pending.push_back((identifier, toast));
                while pending.len() > MAX_PENDING_TOASTS {
                    pending.pop_front();
                }
            }
        }

        fn remove(&self, identifier: &str) {
            if let Ok(mut pending) = self.inner.lock() {
                pending.retain(|(current, _)| current != identifier);
            }
        }
    }

    pub(super) fn permission_state(
        app_id: &str,
    ) -> Result<NativePermissionStatus, NativeNotificationError> {
        let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))
            .map_err(|_| NativeNotificationError::Authorization)?;
        notifier
            .Setting()
            .map(|setting| {
                if setting == NotificationSetting::Enabled {
                    NativePermissionStatus::Granted
                } else {
                    NativePermissionStatus::Denied
                }
            })
            .map_err(|_| NativeNotificationError::Authorization)
    }

    #[allow(clippy::needless_pass_by_value)] // Target ownership must outlive the callback.
    pub(super) fn notify(
        app_id: &str,
        activation: &ActivationHandler,
        delivery_failure: &DeliveryFailureHandler,
        pending_toasts: &PendingToasts,
        notification: NativeNotification,
        identifier: &str,
    ) -> Result<(), NativeNotificationError> {
        let document = XmlDocument::new().map_err(|_| NativeNotificationError::Delivery)?;
        let xml = format!(
            "<toast><visual><binding template=\"ToastGeneric\"><text>{}</text><text>{}</text></binding></visual></toast>",
            escape_xml(&notification.title),
            escape_xml(&notification.body),
        );
        document
            .LoadXml(&HSTRING::from(xml))
            .map_err(|_| NativeNotificationError::Delivery)?;
        let toast = ToastNotification::CreateToastNotification(&document)
            .map_err(|_| NativeNotificationError::Delivery)?;
        toast
            .SetTag(&HSTRING::from(identifier))
            .map_err(|_| NativeNotificationError::Delivery)?;
        toast
            .SetGroup(&HSTRING::from("quotatide-alerts"))
            .map_err(|_| NativeNotificationError::Delivery)?;
        let on_activation = activation.clone();
        let target = notification.target;
        let activation_identifier = identifier.to_owned();
        let activation_toasts = pending_toasts.clone();
        let handler = TypedEventHandler::<ToastNotification, IInspectable>::new(move |_, _| {
            activation_toasts.remove(&activation_identifier);
            on_activation(target);
            Ok(())
        });
        toast
            .Activated(&handler)
            .map_err(|_| NativeNotificationError::Delivery)?;
        let delivery_state = Arc::new(AtomicU8::new(DELIVERY_PENDING));
        let failed_state = delivery_state.clone();
        let failed_identifier = identifier.to_owned();
        let failed_delivery_key = notification.delivery_key;
        let failed_callback = delivery_failure.clone();
        let failed_toasts = pending_toasts.clone();
        let failed_handler =
            TypedEventHandler::<ToastNotification, ToastFailedEventArgs>::new(move |_, _| {
                failed_toasts.remove(&failed_identifier);
                if let Err(DELIVERY_SUBMITTED) = failed_state.compare_exchange(
                    DELIVERY_PENDING,
                    DELIVERY_FAILED_EARLY,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    failed_callback(failed_delivery_key.clone());
                }
                Ok(())
            });
        toast
            .Failed(&failed_handler)
            .map_err(|_| NativeNotificationError::Delivery)?;
        pending_toasts.insert(identifier.to_owned(), toast.clone());
        let show_result =
            ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(app_id))
                .and_then(|notifier| notifier.Show(&toast))
                .map_err(|_| NativeNotificationError::Delivery);
        if show_result.is_err() {
            pending_toasts.remove(identifier);
            return show_result;
        }
        match delivery_state.compare_exchange(
            DELIVERY_PENDING,
            DELIVERY_SUBMITTED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => Ok(()),
            Err(_) => Err(NativeNotificationError::Delivery),
        }
    }

    fn escape_xml(value: &str) -> String {
        value
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identifier_is_secret_free_repeatable_and_routes_target() {
        let notification = NativeNotification {
            delivery_key: "system:event:42".to_owned(),
            title: "额度提醒".to_owned(),
            body: "正文".to_owned(),
            target: NativeAlertTarget::Radar,
        };
        let first = stable_identifier(&notification);
        let second = stable_identifier(&notification);

        assert_eq!(first, second);
        assert!(!first.contains("system:event:42"));
        assert_eq!(
            target_from_identifier(&first),
            Some(NativeAlertTarget::Radar)
        );
    }
}

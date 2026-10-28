//! Deterministic model of BlueZ GATT registration lifecycle.
//!
//! This is not a D-Bus client and cannot register a service or advertisement.

const MAX_EXPERIMENT_SECONDS: u16 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    Manual,
    Timeout,
    Interrupt,
    TerminationSignal,
    AdapterLost,
    RegistrationFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BluezEvent {
    ApplicationRegistered,
    AdvertisementRegistered,
    AdvertisementUnregistered,
    ApplicationUnregistered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GattLifecycleError {
    ExplicitOptInRequired,
    InvalidDuration,
    AdapterUnavailable,
    UnsupportedGattManager,
    UnsupportedAdvertisingManager,
    ApplicationRegistrationFailed,
    AdvertisementRegistrationFailed,
}

#[derive(Debug, Clone)]
pub struct MockBluezManager {
    pub adapter_available: bool,
    pub gatt_manager_supported: bool,
    pub advertising_manager_supported: bool,
    pub fail_application_registration: bool,
    pub fail_advertisement_registration: bool,
    events: Vec<BluezEvent>,
}

impl MockBluezManager {
    pub fn new(gatt_manager_supported: bool, advertising_manager_supported: bool) -> Self {
        Self {
            adapter_available: true,
            gatt_manager_supported,
            advertising_manager_supported,
            fail_application_registration: false,
            fail_advertisement_registration: false,
            events: Vec::new(),
        }
    }

    pub fn events(&self) -> &[BluezEvent] {
        &self.events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MockGattSession {
    timeout_seconds: u16,
    application_registered: bool,
    advertisement_registered: bool,
    active: bool,
    shutdown_reason: Option<ShutdownReason>,
}

impl MockGattSession {
    /// Simulate opt-in and registration. No system call or D-Bus operation is
    /// performed; this state machine exists to test cleanup ordering.
    pub fn start(
        manager: &mut MockBluezManager,
        explicit_opt_in: bool,
        timeout_seconds: u16,
    ) -> Result<Self, GattLifecycleError> {
        if !explicit_opt_in {
            return Err(GattLifecycleError::ExplicitOptInRequired);
        }
        if !(1..=MAX_EXPERIMENT_SECONDS).contains(&timeout_seconds) {
            return Err(GattLifecycleError::InvalidDuration);
        }
        if !manager.adapter_available {
            return Err(GattLifecycleError::AdapterUnavailable);
        }
        if !manager.gatt_manager_supported {
            return Err(GattLifecycleError::UnsupportedGattManager);
        }
        if !manager.advertising_manager_supported {
            return Err(GattLifecycleError::UnsupportedAdvertisingManager);
        }
        if manager.fail_application_registration {
            return Err(GattLifecycleError::ApplicationRegistrationFailed);
        }
        manager.events.push(BluezEvent::ApplicationRegistered);
        if manager.fail_advertisement_registration {
            manager.events.push(BluezEvent::ApplicationUnregistered);
            return Err(GattLifecycleError::AdvertisementRegistrationFailed);
        }
        manager.events.push(BluezEvent::AdvertisementRegistered);
        Ok(Self {
            timeout_seconds,
            application_registered: true,
            advertisement_registered: true,
            active: true,
            shutdown_reason: None,
        })
    }

    pub fn timeout_seconds(&self) -> u16 {
        self.timeout_seconds
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn shutdown_reason(&self) -> Option<ShutdownReason> {
        self.shutdown_reason
    }

    /// Cleanup is idempotent and unregisters advertisement before application.
    pub fn shutdown(&mut self, manager: &mut MockBluezManager, reason: ShutdownReason) {
        if self.advertisement_registered {
            manager.events.push(BluezEvent::AdvertisementUnregistered);
            self.advertisement_registered = false;
        }
        if self.application_registered {
            manager.events.push(BluezEvent::ApplicationUnregistered);
            self.application_registered = false;
        }
        self.active = false;
        self.shutdown_reason.get_or_insert(reason);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supported_manager() -> MockBluezManager {
        MockBluezManager::new(true, true)
    }

    #[test]
    fn requires_opt_in_bounded_time_and_supported_capabilities() {
        let mut manager = supported_manager();
        assert_eq!(
            MockGattSession::start(&mut manager, false, 30),
            Err(GattLifecycleError::ExplicitOptInRequired)
        );
        assert_eq!(
            MockGattSession::start(&mut manager, true, 0),
            Err(GattLifecycleError::InvalidDuration)
        );
        assert_eq!(
            MockGattSession::start(&mut manager, true, 301),
            Err(GattLifecycleError::InvalidDuration)
        );
        manager.adapter_available = false;
        assert_eq!(
            MockGattSession::start(&mut manager, true, 30),
            Err(GattLifecycleError::AdapterUnavailable)
        );
        manager.adapter_available = true;
        manager.gatt_manager_supported = false;
        assert_eq!(
            MockGattSession::start(&mut manager, true, 30),
            Err(GattLifecycleError::UnsupportedGattManager)
        );
        manager.gatt_manager_supported = true;
        manager.advertising_manager_supported = false;
        assert_eq!(
            MockGattSession::start(&mut manager, true, 30),
            Err(GattLifecycleError::UnsupportedAdvertisingManager)
        );
        assert!(manager.events().is_empty());
    }

    #[test]
    fn registration_failures_clean_up_and_shutdown_is_idempotent() {
        let mut manager = supported_manager();
        manager.fail_application_registration = true;
        assert_eq!(
            MockGattSession::start(&mut manager, true, 30),
            Err(GattLifecycleError::ApplicationRegistrationFailed)
        );
        assert!(manager.events().is_empty());

        manager.fail_application_registration = false;
        manager.fail_advertisement_registration = true;
        assert_eq!(
            MockGattSession::start(&mut manager, true, 30),
            Err(GattLifecycleError::AdvertisementRegistrationFailed)
        );
        assert_eq!(
            manager.events(),
            &[
                BluezEvent::ApplicationRegistered,
                BluezEvent::ApplicationUnregistered
            ]
        );

        manager.fail_advertisement_registration = false;
        manager.events.clear();
        let mut session = MockGattSession::start(&mut manager, true, 30).unwrap();
        assert!(session.is_active());
        assert_eq!(session.timeout_seconds(), 30);
        session.shutdown(&mut manager, ShutdownReason::Interrupt);
        session.shutdown(&mut manager, ShutdownReason::AdapterLost);
        assert!(!session.is_active());
        assert_eq!(session.shutdown_reason(), Some(ShutdownReason::Interrupt));
        assert_eq!(
            manager.events(),
            &[
                BluezEvent::ApplicationRegistered,
                BluezEvent::AdvertisementRegistered,
                BluezEvent::AdvertisementUnregistered,
                BluezEvent::ApplicationUnregistered
            ]
        );

        for reason in [
            ShutdownReason::Manual,
            ShutdownReason::Timeout,
            ShutdownReason::Interrupt,
            ShutdownReason::TerminationSignal,
            ShutdownReason::AdapterLost,
        ] {
            let mut isolated_manager = supported_manager();
            let mut isolated_session =
                MockGattSession::start(&mut isolated_manager, true, 30).unwrap();
            isolated_session.shutdown(&mut isolated_manager, reason);
            assert!(!isolated_session.is_active());
            assert_eq!(isolated_session.shutdown_reason(), Some(reason));
            assert_eq!(
                &isolated_manager.events()[2..],
                &[
                    BluezEvent::AdvertisementUnregistered,
                    BluezEvent::ApplicationUnregistered
                ]
            );
        }
    }
}

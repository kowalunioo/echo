//! The Windows microphone privacy check (`microphone.md` rule 7).
//!
//! Windows keeps its microphone privacy switches in the registry under the capability access
//! manager's consent store: the device-wide "Microphone access" switch (machine), "Let apps
//! access your microphone" (user) and "Let desktop apps access your microphone" (user,
//! `NonPackaged`), each stored as `Value` = `Allow` or `Deny`; an administrator policy can also
//! force access off. Echo is a desktop app, so any of them set to deny blocks it.

use super::MicrophoneAccess;

/// The privacy switches as read from the registry; `None` where a value is absent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivacySwitches {
    /// "Microphone access" for the whole device.
    pub device: Option<String>,
    /// "Let apps access your microphone" for this user.
    pub apps: Option<String>,
    /// "Let desktop apps access your microphone" for this user.
    pub desktop_apps: Option<String>,
    /// The `LetAppsAccessMicrophone` policy: 0 user decides, 1 force allow, 2 force deny.
    pub policy: Option<u32>,
}

/// Decides access from the switches. Absent values mean Windows' default, which is allowed.
pub fn access_from_switches(switches: &PrivacySwitches) -> MicrophoneAccess {
    let denies = |value: &Option<String>| {
        value
            .as_deref()
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("deny"))
    };
    // A policy that forces access on overrides the user's own switches, not the device switch.
    let user_blocks =
        switches.policy != Some(1) && (denies(&switches.apps) || denies(&switches.desktop_apps));
    let blocked = switches.policy == Some(2) || denies(&switches.device) || user_blocks;
    if blocked {
        MicrophoneAccess::Denied
    } else {
        MicrophoneAccess::Allowed
    }
}

/// Reads the switches of this computer and user.
#[cfg(windows)]
pub fn read_switches() -> PrivacySwitches {
    use windows_registry::{CURRENT_USER, LOCAL_MACHINE};

    const CONSENT: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";
    const POLICY: &str = r"SOFTWARE\Policies\Microsoft\Windows\AppPrivacy";

    let string = |root: &windows_registry::Key, path: &str| {
        root.open(path).and_then(|key| key.get_string("Value")).ok()
    };
    PrivacySwitches {
        device: string(LOCAL_MACHINE, CONSENT),
        apps: string(CURRENT_USER, CONSENT),
        desktop_apps: string(CURRENT_USER, &format!(r"{CONSENT}\NonPackaged")),
        policy: LOCAL_MACHINE
            .open(POLICY)
            .and_then(|key| key.get_u32("LetAppsAccessMicrophone"))
            .ok(),
    }
}

/// Other platforms have no such switches yet.
#[cfg(not(windows))]
pub fn read_switches() -> PrivacySwitches {
    PrivacySwitches::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deny() -> Option<String> {
        Some("Deny".into())
    }

    #[test]
    fn nothing_set_or_everything_allowed_means_allowed() {
        assert_eq!(
            access_from_switches(&PrivacySwitches::default()),
            MicrophoneAccess::Allowed
        );
        let allow = Some("Allow".to_owned());
        let all_allowed = PrivacySwitches {
            device: allow.clone(),
            apps: allow.clone(),
            desktop_apps: allow,
            policy: Some(0),
        };
        assert_eq!(
            access_from_switches(&all_allowed),
            MicrophoneAccess::Allowed
        );
    }

    #[test]
    fn any_switch_set_to_deny_blocks_desktop_apps() {
        for switches in [
            PrivacySwitches {
                device: deny(),
                ..Default::default()
            },
            PrivacySwitches {
                apps: deny(),
                ..Default::default()
            },
            PrivacySwitches {
                desktop_apps: Some("deny".into()),
                ..Default::default()
            },
            PrivacySwitches {
                policy: Some(2),
                ..Default::default()
            },
        ] {
            assert_eq!(
                access_from_switches(&switches),
                MicrophoneAccess::Denied,
                "{switches:?}"
            );
        }
    }

    #[test]
    fn a_policy_that_forces_access_on_overrides_the_user_switches() {
        let switches = PrivacySwitches {
            apps: deny(),
            desktop_apps: deny(),
            policy: Some(1),
            ..Default::default()
        };
        assert_eq!(access_from_switches(&switches), MicrophoneAccess::Allowed);
        let device_off = PrivacySwitches {
            device: deny(),
            policy: Some(1),
            ..Default::default()
        };
        assert_eq!(access_from_switches(&device_off), MicrophoneAccess::Denied);
    }
}

//! The in-app updater (`docs/specs/updater.md`).
//!
//! [`UpdaterCore`] is the whole behaviour: when to check, what to show, when an
//! automatic update may install. It is pure and clock-injected: the caller passes
//! the time to every call and the outside world comes in through three seams —
//! [`UpdateFeed`] (the release feed: check and verified download), [`Installer`]
//! (install and restart) and [`Host`] (Idle, the setting, the restart marker and
//! publishing the status). `app.rs` drives it once a second on its own thread.

pub mod app;
pub mod feed;
pub mod marker;

#[cfg(test)]
mod feed_tests;
#[cfg(test)]
mod test_server;
#[cfg(test)]
mod tests;

use std::time::Duration;

use semver::Version;
use serde::{Deserialize, Serialize};
use specta::Type;

/// The first automatic check runs this long after start (rule 4).
pub const FIRST_CHECK_AFTER: Duration = Duration::from_secs(30);
/// Automatic checks repeat this often while Echo runs (rule 4).
pub const CHECK_INTERVAL: Duration = Duration::from_secs(4 * 60 * 60);
/// An automatic install waits until Echo has been Idle this long (rule 5).
pub const IDLE_BEFORE_INSTALL: Duration = Duration::from_secs(10);
/// "Echo is up to date" shows this long (rule 7).
pub const UP_TO_DATE_FOR: Duration = Duration::from_secs(3);

/// A release the feed offers.
#[derive(Debug, Clone)]
pub struct Found<R> {
    pub version: Version,
    pub release: R,
}

/// Why a check failed (network or server error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckError(pub String);

/// Why a download failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadError {
    /// The package's signature is missing or does not verify against Echo's key (rule 2).
    Unverified(String),
    /// Network or server error.
    Failed(String),
}

/// The release feed: newer signed releases and their packages.
pub trait UpdateFeed {
    /// What the installer needs to know about a release.
    type Release;
    /// A downloaded package whose signature has been verified.
    type Package;

    /// The release the feed offers, if it is newer than the running version.
    fn check(&self) -> Result<Option<Found<Self::Release>>, CheckError>;

    /// Downloads the release's package and verifies its signature. `progress`
    /// receives the bytes received so far and the total, when known.
    fn download(
        &self,
        release: &Self::Release,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Self::Package, DownloadError>;
}

/// Installs a verified package and restarts Echo.
pub trait Installer<R, P> {
    fn install(&self, release: &R, package: P) -> Result<(), String>;
}

/// What the updater needs from the rest of Echo.
pub trait Host {
    /// No Recording, Transcribing or Inserting and no Model download running (rule 5).
    fn is_idle(&self) -> bool;
    /// The "Check for updates automatically" setting (rule 10).
    fn automatic(&self) -> bool;
    /// Called right before installing: remember what to restore after the restart (rule 6).
    fn before_install(&self, version: &Version);
    /// The status shown in the main window changed.
    fn publish(&self, status: &UpdateStatus);
}

/// What the main window's update status line shows (spec, UI section).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// Nothing to show.
    Idle,
    /// A manual check is running.
    Checking,
    /// A manual check found nothing newer; shows for [`UP_TO_DATE_FOR`].
    UpToDate,
    /// A newer version waits for the user to confirm "Install and restart".
    Available { version: String },
    /// A package is downloading.
    Downloading {
        version: String,
        /// Whole percent, when the size is known.
        percent: Option<u8>,
    },
    /// An automatic update is downloaded and installs when Echo has been Idle for 10 s.
    Ready { version: String },
    /// The package is being installed; Echo restarts next.
    Installing { version: String },
    /// A manual check failed: "Couldn't check for updates" (rule 9).
    CheckFailed,
    /// The package's signature did not verify: "Update could not be verified" (rule 2).
    Unverified,
    /// Downloading the package failed.
    DownloadFailed,
    /// Starting the installer failed.
    InstallFailed,
}

enum Pending<R, P> {
    /// Found by a check, not downloaded yet.
    Found { version: Version, release: R },
    /// Downloaded and verified; `automatic` installs it on its own after 10 s Idle.
    Downloaded {
        version: Version,
        release: R,
        package: P,
        automatic: bool,
    },
}

impl<R, P> Pending<R, P> {
    fn version(&self) -> &Version {
        match self {
            Pending::Found { version, .. } | Pending::Downloaded { version, .. } => version,
        }
    }
}

/// The updater's behaviour; see the module documentation.
pub struct UpdaterCore<F: UpdateFeed, I> {
    feed: F,
    installer: I,
    current: Version,
    next_check: Duration,
    idle_since: Option<Duration>,
    up_to_date_until: Option<Duration>,
    status: UpdateStatus,
    pending: Option<Pending<F::Release, F::Package>>,
}

impl<F, I> UpdaterCore<F, I>
where
    F: UpdateFeed,
    I: Installer<F::Release, F::Package>,
{
    /// `started` is the time Echo started; the first automatic check is 30 s later.
    pub fn new(feed: F, installer: I, current: Version, started: Duration) -> Self {
        Self {
            feed,
            installer,
            current,
            next_check: started + FIRST_CHECK_AFTER,
            idle_since: None,
            up_to_date_until: None,
            status: UpdateStatus::Idle,
            pending: None,
        }
    }

    pub fn status(&self) -> &UpdateStatus {
        &self.status
    }

    /// Runs the schedule; call it about once a second.
    pub fn tick(&mut self, now: Duration, host: &impl Host) {
        if host.is_idle() {
            self.idle_since.get_or_insert(now);
        } else {
            self.idle_since = None;
        }
        if self.up_to_date_until.is_some_and(|until| now >= until) {
            self.up_to_date_until = None;
            self.set(UpdateStatus::Idle, host);
        }
        let automatic = host.automatic();
        if !automatic {
            self.stop_automatic_install(host);
        }
        if automatic && now >= self.next_check {
            self.next_check = now + CHECK_INTERVAL;
            self.check_automatically(host);
        }
        let idle_long_enough = self
            .idle_since
            .is_some_and(|since| now.saturating_sub(since) >= IDLE_BEFORE_INSTALL);
        if automatic
            && idle_long_enough
            && matches!(
                self.pending,
                Some(Pending::Downloaded {
                    automatic: true,
                    ..
                })
            )
        {
            self.install(host);
        }
    }

    /// "Check for updates" from the tray or the settings (rule 7).
    pub fn check_manually(&mut self, now: Duration, host: &impl Host) {
        if let Some(pending) = &self.pending {
            // Already found or downloaded: offer it without asking the feed again.
            let version = pending.version().to_string();
            self.stop_automatic_install(host);
            self.set(UpdateStatus::Available { version }, host);
            return;
        }
        self.up_to_date_until = None;
        self.set(UpdateStatus::Checking, host);
        match self.feed.check() {
            Err(CheckError(error)) => {
                log::warn!("Manual update check failed: {error}");
                self.set(UpdateStatus::CheckFailed, host);
            }
            Ok(Some(found)) if found.version > self.current => {
                log::info!("Update {} is available", found.version);
                let version = found.version.to_string();
                self.pending = Some(Pending::Found {
                    version: found.version,
                    release: found.release,
                });
                self.set(UpdateStatus::Available { version }, host);
            }
            Ok(_) => {
                self.up_to_date_until = Some(now + UP_TO_DATE_FOR);
                self.set(UpdateStatus::UpToDate, host);
            }
        }
    }

    /// The user confirmed "Install and restart" (rule 7).
    pub fn install_confirmed(&mut self, host: &impl Host) {
        match self.pending.take() {
            Some(Pending::Found { version, release }) => {
                if let Some(package) = self.download(&version, &release, true, host) {
                    self.pending = Some(Pending::Downloaded {
                        version,
                        release,
                        package,
                        automatic: false,
                    });
                    self.install(host);
                }
            }
            Some(downloaded @ Pending::Downloaded { .. }) => {
                self.pending = Some(downloaded);
                self.install(host);
            }
            None => {}
        }
    }

    fn check_automatically(&mut self, host: &impl Host) {
        if matches!(self.pending, Some(Pending::Downloaded { .. })) {
            return;
        }
        let found = match self.feed.check() {
            Ok(found) => found,
            Err(CheckError(error)) => {
                // Rule 9: silent for automatic checks.
                log::warn!("Automatic update check failed: {error}");
                return;
            }
        };
        let Some(found) = found.filter(|found| found.version > self.current) else {
            log::info!("Echo {} is up to date", self.current);
            return;
        };
        log::info!("Downloading update {} in the background", found.version);
        if let Some(package) = self.download(&found.version, &found.release, false, host) {
            let version = found.version.to_string();
            self.pending = Some(Pending::Downloaded {
                version: found.version,
                release: found.release,
                package,
                automatic: true,
            });
            self.set(UpdateStatus::Ready { version }, host);
        }
    }

    fn download(
        &mut self,
        version: &Version,
        release: &F::Release,
        manual: bool,
        host: &impl Host,
    ) -> Option<F::Package> {
        let shown = version.to_string();
        self.set(
            UpdateStatus::Downloading {
                version: shown.clone(),
                percent: None,
            },
            host,
        );
        let mut last = None;
        let mut progress = |received: u64, total: Option<u64>| {
            let percent = total
                .filter(|total| *total > 0)
                .map(|total| (received.min(total) * 100 / total) as u8);
            if percent != last {
                last = percent;
                host.publish(&UpdateStatus::Downloading {
                    version: shown.clone(),
                    percent,
                });
            }
        };
        let result = self.feed.download(release, &mut progress);
        if let Some(percent) = last {
            self.status = UpdateStatus::Downloading {
                version: shown,
                percent: Some(percent),
            };
        }
        match result {
            Ok(package) => Some(package),
            Err(DownloadError::Unverified(error)) => {
                log::warn!("Update {version} could not be verified: {error}");
                self.set(UpdateStatus::Unverified, host);
                None
            }
            Err(DownloadError::Failed(error)) => {
                log::warn!("Downloading update {version} failed: {error}");
                let status = if manual {
                    UpdateStatus::DownloadFailed
                } else {
                    UpdateStatus::Idle
                };
                self.set(status, host);
                None
            }
        }
    }

    fn install(&mut self, host: &impl Host) {
        let Some(Pending::Downloaded {
            version,
            release,
            package,
            ..
        }) = self.pending.take()
        else {
            return;
        };
        log::info!("Installing update {version} and restarting");
        self.set(
            UpdateStatus::Installing {
                version: version.to_string(),
            },
            host,
        );
        host.before_install(&version);
        if let Err(error) = self.installer.install(&release, package) {
            log::error!("Installing update {version} failed: {error}");
            self.set(UpdateStatus::InstallFailed, host);
        }
    }

    /// With automatic checks off, a downloaded update waits for the user instead.
    fn stop_automatic_install(&mut self, host: &impl Host) {
        if let Some(Pending::Downloaded {
            automatic, version, ..
        }) = &mut self.pending
            && *automatic
        {
            *automatic = false;
            let version = version.to_string();
            self.set(UpdateStatus::Available { version }, host);
        }
    }

    fn set(&mut self, status: UpdateStatus, host: &impl Host) {
        if self.status != status {
            self.status = status;
            host.publish(&self.status);
        }
    }
}

/// Rule 11: `ECHO_DISABLE_UPDATES` turns the updater off for managed installs.
pub const DISABLE_ENV: &str = "ECHO_DISABLE_UPDATES";

/// Whether the updater is disabled by the environment. Any value except empty,
/// `0` and `false` disables it.
pub fn disabled_by_env(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> bool {
    env(DISABLE_ENV).is_some_and(|value| {
        let value = value.to_string_lossy();
        let value = value.trim();
        !(value.is_empty() || value == "0" || value.eq_ignore_ascii_case("false"))
    })
}

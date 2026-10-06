//! The updater's behaviour with a fake feed, installer and host (`updater.md`).

use std::cell::{Cell, RefCell};
use std::time::Duration;

use semver::Version;

use super::*;

const SECOND: Duration = Duration::from_secs(1);

#[derive(Default)]
struct FakeFeed {
    offered: RefCell<Option<&'static str>>,
    check_fails: Cell<bool>,
    download: RefCell<Option<DownloadError>>,
    checks: Cell<usize>,
    downloads: Cell<usize>,
}

impl FakeFeed {
    fn offering(version: &'static str) -> Self {
        let feed = Self::default();
        feed.offered.replace(Some(version));
        feed
    }
}

impl UpdateFeed for &FakeFeed {
    type Release = String;
    type Package = String;

    fn check(&self) -> Result<Option<Found<String>>, CheckError> {
        self.checks.set(self.checks.get() + 1);
        if self.check_fails.get() {
            return Err(CheckError("500".into()));
        }
        Ok(self.offered.borrow().map(|version| Found {
            version: Version::parse(version).unwrap(),
            release: format!("release {version}"),
        }))
    }

    fn download(
        &self,
        release: &String,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<String, DownloadError> {
        self.downloads.set(self.downloads.get() + 1);
        progress(50, Some(100));
        progress(100, Some(100));
        match self.download.borrow().clone() {
            Some(error) => Err(error),
            None => Ok(format!("package of {release}")),
        }
    }
}

#[derive(Default)]
struct FakeInstaller {
    installed: RefCell<Vec<String>>,
}

impl Installer<String, String> for &FakeInstaller {
    fn install(&self, _release: &String, package: String) -> Result<(), String> {
        self.installed.borrow_mut().push(package);
        Ok(())
    }
}

struct FakeHost {
    idle: Cell<bool>,
    automatic: Cell<bool>,
    prepared: RefCell<Vec<String>>,
    published: RefCell<Vec<UpdateStatus>>,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self {
            idle: Cell::new(true),
            automatic: Cell::new(true),
            prepared: RefCell::default(),
            published: RefCell::default(),
        }
    }
}

impl Host for FakeHost {
    fn is_idle(&self) -> bool {
        self.idle.get()
    }
    fn automatic(&self) -> bool {
        self.automatic.get()
    }
    fn before_install(&self, version: &Version) {
        self.prepared.borrow_mut().push(version.to_string());
    }
    fn publish(&self, status: &UpdateStatus) {
        self.published.borrow_mut().push(status.clone());
    }
}

fn core<'a>(
    feed: &'a FakeFeed,
    installer: &'a FakeInstaller,
) -> UpdaterCore<&'a FakeFeed, &'a FakeInstaller> {
    UpdaterCore::new(feed, installer, Version::new(0, 1, 0), Duration::ZERO)
}

/// Ticks once a second from `from` up to and including `to`.
fn run(
    updater: &mut UpdaterCore<&FakeFeed, &FakeInstaller>,
    host: &FakeHost,
    from: Duration,
    to: Duration,
) {
    let mut now = from;
    while now <= to {
        updater.tick(now, host);
        now += SECOND;
    }
}

fn available(version: &str) -> UpdateStatus {
    UpdateStatus::Available {
        version: version.into(),
    }
}

#[test]
fn rule_4_the_first_automatic_check_is_30_s_after_start_then_every_4_hours() {
    let feed = FakeFeed::default();
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    run(&mut updater, &host, Duration::ZERO, 29 * SECOND);
    assert_eq!(feed.checks.get(), 0);
    updater.tick(30 * SECOND, &host);
    assert_eq!(feed.checks.get(), 1);

    let next = 30 * SECOND + CHECK_INTERVAL;
    updater.tick(next - SECOND, &host);
    assert_eq!(feed.checks.get(), 1);
    updater.tick(next, &host);
    assert_eq!(feed.checks.get(), 2);
}

#[test]
fn acceptance_1_manual_check_up_to_date_shows_for_3_s() {
    let feed = FakeFeed::offering("0.1.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    assert_eq!(
        *host.published.borrow(),
        vec![UpdateStatus::Checking, UpdateStatus::UpToDate]
    );
    // The 3 s count from the first tick after the result.
    updater.tick(2 * SECOND, &host);
    updater.tick(4 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    updater.tick(5 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::Idle);
}

#[test]
fn up_to_date_still_shows_for_3_s_when_the_feed_answers_slowly() {
    // The check started at 1 s, but the feed took 4 s to answer, so the first tick after
    // the result is at 5 s; the user must still see "up to date" for 3 s from then.
    let feed = FakeFeed::offering("0.1.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    updater.tick(5 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    updater.tick(7 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    updater.tick(8 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::Idle);
}

#[test]
fn acceptance_2_manual_install_happens_only_after_confirmation() {
    let feed = FakeFeed::offering("0.2.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    assert_eq!(updater.status(), &available("0.2.0"));
    assert_eq!(feed.downloads.get(), 0);
    // Idle for a long time: a manual find never installs on its own.
    run(&mut updater, &host, 2 * SECOND, 25 * SECOND);
    assert!(installer.installed.borrow().is_empty());

    updater.install_confirmed(&host);
    assert_eq!(
        *installer.installed.borrow(),
        vec!["package of release 0.2.0"]
    );
    assert_eq!(*host.prepared.borrow(), vec!["0.2.0"]);
    let published = host.published.borrow();
    let tail: Vec<_> = published.iter().skip(2).cloned().collect();
    assert_eq!(
        tail,
        vec![
            UpdateStatus::Downloading {
                version: "0.2.0".into(),
                percent: None
            },
            UpdateStatus::Downloading {
                version: "0.2.0".into(),
                percent: Some(50)
            },
            UpdateStatus::Downloading {
                version: "0.2.0".into(),
                percent: Some(100)
            },
            UpdateStatus::Installing {
                version: "0.2.0".into()
            },
        ]
    );
}

#[test]
fn acceptance_3_an_unverified_package_is_never_installed() {
    let feed = FakeFeed::offering("0.2.0");
    feed.download
        .replace(Some(DownloadError::Unverified("bad signature".into())));
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    updater.install_confirmed(&host);
    assert_eq!(updater.status(), &UpdateStatus::Unverified);
    assert!(installer.installed.borrow().is_empty());
    assert!(host.prepared.borrow().is_empty());

    // An automatic check meets the same package: told, not installed.
    run(&mut updater, &host, 2 * SECOND, 60 * SECOND);
    assert_eq!(updater.status(), &UpdateStatus::Unverified);
    assert!(installer.installed.borrow().is_empty());
}

#[test]
fn acceptance_4_an_older_version_is_never_offered() {
    let feed = FakeFeed::offering("0.0.9");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    run(&mut updater, &host, 2 * SECOND, 60 * SECOND);
    assert_eq!(feed.downloads.get(), 0);
    assert!(installer.installed.borrow().is_empty());
}

#[test]
fn acceptance_5_automatic_install_waits_for_10_s_idle() {
    let feed = FakeFeed::offering("0.2.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    // A Recording is in progress when the automatic check downloads the update.
    host.idle.set(false);
    run(&mut updater, &host, Duration::ZERO, 30 * SECOND);
    assert_eq!(
        updater.status(),
        &UpdateStatus::Ready {
            version: "0.2.0".into()
        }
    );
    run(&mut updater, &host, 31 * SECOND, 120 * SECOND);
    assert!(installer.installed.borrow().is_empty());

    // The Dictation ends; a short Idle spell is interrupted by another Dictation.
    host.idle.set(true);
    run(&mut updater, &host, 121 * SECOND, 125 * SECOND);
    host.idle.set(false);
    updater.tick(126 * SECOND, &host);
    host.idle.set(true);
    run(&mut updater, &host, 127 * SECOND, 136 * SECOND);
    assert!(installer.installed.borrow().is_empty());
    updater.tick(137 * SECOND, &host);
    assert_eq!(installer.installed.borrow().len(), 1);
    assert_eq!(*host.prepared.borrow(), vec!["0.2.0"]);
}

#[test]
fn acceptance_6_with_automatic_checks_off_only_manual_checks_reach_the_feed() {
    let feed = FakeFeed::offering("0.2.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    host.automatic.set(false);
    let mut updater = core(&feed, &installer);

    run(&mut updater, &host, Duration::ZERO, 60 * SECOND);
    updater.tick(CHECK_INTERVAL + 60 * SECOND, &host);
    assert_eq!(feed.checks.get(), 0);

    updater.check_manually(&host);
    assert_eq!(feed.checks.get(), 1);
    assert_eq!(updater.status(), &available("0.2.0"));
}

#[test]
fn turning_automatic_checks_off_stops_a_waiting_automatic_install() {
    let feed = FakeFeed::offering("0.2.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    host.idle.set(false);
    let mut updater = core(&feed, &installer);
    run(&mut updater, &host, Duration::ZERO, 30 * SECOND);

    host.automatic.set(false);
    host.idle.set(true);
    run(&mut updater, &host, 31 * SECOND, 60 * SECOND);
    assert!(installer.installed.borrow().is_empty());
    assert_eq!(updater.status(), &available("0.2.0"));

    // Confirming installs the package already downloaded.
    updater.install_confirmed(&host);
    assert_eq!(feed.downloads.get(), 1);
    assert_eq!(installer.installed.borrow().len(), 1);
}

#[test]
fn acceptance_8_automatic_check_errors_are_silent_manual_ones_are_shown() {
    let feed = FakeFeed::offering("0.2.0");
    feed.check_fails.set(true);
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    run(&mut updater, &host, Duration::ZERO, 31 * SECOND);
    assert_eq!(feed.checks.get(), 1);
    assert!(host.published.borrow().is_empty());

    updater.check_manually(&host);
    assert_eq!(updater.status(), &UpdateStatus::CheckFailed);
}

#[test]
fn a_failed_automatic_download_is_silent_and_retried_at_the_next_check() {
    let feed = FakeFeed::offering("0.2.0");
    feed.download
        .replace(Some(DownloadError::Failed("connection reset".into())));
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    run(&mut updater, &host, Duration::ZERO, 30 * SECOND);
    assert_eq!(updater.status(), &UpdateStatus::Idle);

    feed.download.replace(None);
    // Idle all along: the retried download installs right away.
    updater.tick(30 * SECOND + CHECK_INTERVAL, &host);
    assert_eq!(feed.downloads.get(), 2);
    assert_eq!(installer.installed.borrow().len(), 1);
}

#[test]
fn a_failed_manual_download_is_shown() {
    let feed = FakeFeed::offering("0.2.0");
    feed.download
        .replace(Some(DownloadError::Failed("connection reset".into())));
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    let mut updater = core(&feed, &installer);

    updater.check_manually(&host);
    updater.install_confirmed(&host);
    assert_eq!(updater.status(), &UpdateStatus::DownloadFailed);
    assert!(installer.installed.borrow().is_empty());
}

#[test]
fn a_manual_check_offers_an_update_already_downloaded_without_asking_again() {
    let feed = FakeFeed::offering("0.2.0");
    let installer = FakeInstaller::default();
    let host = FakeHost::default();
    host.idle.set(false);
    let mut updater = core(&feed, &installer);
    run(&mut updater, &host, Duration::ZERO, 30 * SECOND);

    updater.check_manually(&host);
    assert_eq!(feed.checks.get(), 1);
    assert_eq!(updater.status(), &available("0.2.0"));
    updater.install_confirmed(&host);
    assert_eq!(installer.installed.borrow().len(), 1);
}

#[test]
fn rule_11_the_environment_variable_disables_the_updater() {
    let env = |value: &'static str| move |key: &str| (key == DISABLE_ENV).then(|| value.into());
    assert!(disabled_by_env(env("1")));
    assert!(disabled_by_env(env("true")));
    assert!(!disabled_by_env(env("")));
    assert!(!disabled_by_env(env("0")));
    assert!(!disabled_by_env(env("FALSE")));
    assert!(!disabled_by_env(|_| None));
}

//! Acceptance tests 1–4, 6 and 8 of `updater.md` through the real feed
//! (`tauri-plugin-updater` in a mock app) against a local HTTP server that serves
//! an update manifest and a package signed with a throwaway test key.
//!
//! Fixtures: `fixtures/package.bin`, its signature by the test key
//! (`package.bin.sig`, public key `test-key.pub`) and a signature by another key
//! (`package.bin.other-key.sig`). The private keys were thrown away.

use std::cell::RefCell;
use std::time::Duration;

use semver::Version;
use serde_json::json;
use tauri::test::{MockRuntime, mock_builder, mock_context, noop_assets};

use super::feed::PluginFeed;
use super::test_server::{Response, TestServer};
use super::*;

const TEST_PUBKEY: &str = include_str!("fixtures/test-key.pub");
const PACKAGE: &[u8] = include_bytes!("fixtures/package.bin");
const SIGNATURE: &str = include_str!("fixtures/package.bin.sig");
const OTHER_KEY_SIGNATURE: &str = include_str!("fixtures/package.bin.other-key.sig");
const SECOND: Duration = Duration::from_secs(1);

fn mock_app() -> tauri::App<MockRuntime> {
    let mut context = mock_context(noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({ "pubkey": TEST_PUBKEY.trim(), "dangerousInsecureTransportProtocol": true }),
    );
    context.package_info_mut().version = Version::new(0, 1, 0);
    mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap()
}

/// Serves a manifest announcing `version`, whose package carries `signature`.
fn serve(server: &TestServer, version: &str, signature: &str) {
    let manifest = json!({
        "version": version,
        "notes": "test release",
        "pub_date": "2026-10-01T00:00:00Z",
        "platforms": {
            "windows-x86_64": { "signature": signature.trim(), "url": server.url("/package.bin") }
        }
    });
    server.route("/latest.json", Response::ok(manifest.to_string()));
    server.route("/package.bin", Response::ok(PACKAGE));
}

#[derive(Default)]
struct RecordingInstaller {
    installed: RefCell<Vec<Vec<u8>>>,
}

impl<R> Installer<R, Vec<u8>> for &RecordingInstaller {
    fn install(&self, _release: &R, package: Vec<u8>) -> Result<(), String> {
        self.installed.borrow_mut().push(package);
        Ok(())
    }
}

struct Host {
    automatic: bool,
    prepared: RefCell<Vec<String>>,
}

impl Host {
    fn new(automatic: bool) -> Self {
        Self {
            automatic,
            prepared: RefCell::default(),
        }
    }
}

impl super::Host for Host {
    fn is_idle(&self) -> bool {
        true
    }
    fn automatic(&self) -> bool {
        self.automatic
    }
    fn before_install(&self, version: &Version) {
        self.prepared.borrow_mut().push(version.to_string());
    }
    fn publish(&self, _status: &UpdateStatus) {}
}

fn updater<'a>(
    app: &tauri::App<MockRuntime>,
    server: &TestServer,
    installer: &'a RecordingInstaller,
) -> UpdaterCore<PluginFeed<MockRuntime>, &'a RecordingInstaller> {
    let endpoint = server.url("/latest.json").parse().unwrap();
    let feed = PluginFeed::with_endpoint(app.handle().clone(), endpoint);
    UpdaterCore::new(feed, installer, Version::new(0, 1, 0), Duration::ZERO)
}

#[test]
fn acceptance_1_same_version_is_up_to_date() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.1.0", SIGNATURE);
    let mut updater = updater(&app, &server, &installer);

    updater.check_manually(SECOND, &Host::new(true));
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    assert_eq!(server.received(), vec!["/latest.json"]);
}

#[test]
fn acceptance_2_a_newer_signed_version_installs_after_confirmation() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.2.0", SIGNATURE);
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    updater.check_manually(SECOND, &host);
    assert_eq!(
        updater.status(),
        &UpdateStatus::Available {
            version: "0.2.0".into()
        }
    );
    assert!(installer.installed.borrow().is_empty());

    updater.install_confirmed(&host);
    assert_eq!(*installer.installed.borrow(), vec![PACKAGE.to_vec()]);
    assert_eq!(*host.prepared.borrow(), vec!["0.2.0"]);
    assert_eq!(server.received(), vec!["/latest.json", "/package.bin"]);
}

#[test]
fn acceptance_3_a_package_signed_with_another_key_is_rejected() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.2.0", OTHER_KEY_SIGNATURE);
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    updater.check_manually(SECOND, &host);
    updater.install_confirmed(&host);
    assert_eq!(updater.status(), &UpdateStatus::Unverified);
    assert!(installer.installed.borrow().is_empty());
    assert!(host.prepared.borrow().is_empty());
}

#[test]
fn acceptance_3_a_missing_signature_is_rejected() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.2.0", "");
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    updater.check_manually(SECOND, &host);
    updater.install_confirmed(&host);
    assert_eq!(updater.status(), &UpdateStatus::Unverified);
    assert!(installer.installed.borrow().is_empty());
}

#[test]
fn acceptance_4_an_older_version_is_not_offered() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.0.9", SIGNATURE);
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    updater.check_manually(SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::UpToDate);
    // Nor installed by an automatic check.
    for second in 2..=40 {
        updater.tick(second * SECOND, &host);
    }
    assert!(installer.installed.borrow().is_empty());
    assert!(!server.received().contains(&"/package.bin".to_owned()));
}

#[test]
fn acceptance_6_with_automatic_checks_off_no_request_reaches_the_server() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.2.0", SIGNATURE);
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(false);

    for second in 0..=60 {
        updater.tick(second * SECOND, &host);
    }
    updater.tick(CHECK_INTERVAL + 31 * SECOND, &host);
    assert!(server.received().is_empty());

    updater.check_manually(CHECK_INTERVAL + 32 * SECOND, &host);
    assert_eq!(server.received(), vec!["/latest.json"]);
    assert_eq!(
        updater.status(),
        &UpdateStatus::Available {
            version: "0.2.0".into()
        }
    );
}

#[test]
fn an_automatic_check_downloads_and_installs_when_idle() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    serve(&server, "0.2.0", SIGNATURE);
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    for second in 0..=40 {
        updater.tick(second * SECOND, &host);
    }
    assert_eq!(*installer.installed.borrow(), vec![PACKAGE.to_vec()]);
}

#[test]
fn acceptance_8_a_server_error_is_silent_when_automatic_and_shown_when_manual() {
    let (app, server, installer) = (
        mock_app(),
        TestServer::start(),
        RecordingInstaller::default(),
    );
    server.route("/latest.json", Response::status(500));
    let mut updater = updater(&app, &server, &installer);
    let host = Host::new(true);

    for second in 0..=31 {
        updater.tick(second * SECOND, &host);
    }
    assert_eq!(server.received(), vec!["/latest.json"]);
    assert_eq!(updater.status(), &UpdateStatus::Idle);

    updater.check_manually(32 * SECOND, &host);
    assert_eq!(updater.status(), &UpdateStatus::CheckFailed);
}

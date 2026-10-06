//! The real release feed and installer, over `tauri-plugin-updater`.
//!
//! The plugin fetches the update manifest (`latest.json` of the latest GitHub
//! release), offers only newer versions, and verifies the package's minisign
//! signature against the public key in `tauri.conf.json` while downloading.

use std::sync::Arc;
use std::time::Duration;

use semver::Version;
use tauri::{AppHandle, Runtime, Url};
use tauri_plugin_updater::{Error, Update, UpdaterExt};

use super::{CheckError, DownloadError, Found, Installer, UpdateFeed};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
/// Development builds only: the update manifest URL to use instead of the real feed.
pub const DEV_ENDPOINT_ENV: &str = "ECHO_UPDATER_ENDPOINT";

/// The release feed of Echo's public repository.
pub struct PluginFeed<R: Runtime> {
    app: AppHandle<R>,
    /// Replaces the endpoints from `tauri.conf.json` (tests use a local server).
    endpoints: Option<Vec<Url>>,
    /// Runs after the installer is ready and right before the plugin ends Echo.
    on_before_exit: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl<R: Runtime> PluginFeed<R> {
    /// `on_before_exit` runs only when the installer really is about to end Echo.
    pub fn new(app: AppHandle<R>, on_before_exit: impl Fn() + Send + Sync + 'static) -> Self {
        // Development builds can point the updater at a local feed to try the UI end to end.
        let endpoints = cfg!(debug_assertions)
            .then(|| std::env::var(DEV_ENDPOINT_ENV).ok())
            .flatten()
            .and_then(|url| url.parse().ok())
            .map(|url| vec![url]);
        Self {
            app,
            endpoints,
            on_before_exit: Some(Arc::new(on_before_exit)),
        }
    }

    #[cfg(test)]
    pub fn with_endpoint(app: AppHandle<R>, endpoint: Url) -> Self {
        Self {
            app,
            endpoints: Some(vec![endpoint]),
            on_before_exit: None,
        }
    }

    fn updater(&self) -> Result<tauri_plugin_updater::Updater, Error> {
        let mut builder = self
            .app
            .updater_builder()
            // Rule 3: only newer versions, never a downgrade.
            .version_comparator(|current, release| release.version > current)
            .configure_client(|client| {
                client
                    .connect_timeout(CONNECT_TIMEOUT)
                    .read_timeout(READ_TIMEOUT)
            });
        if let Some(endpoints) = &self.endpoints {
            builder = builder.endpoints(endpoints.clone())?;
        }
        if let Some(on_before_exit) = &self.on_before_exit {
            let on_before_exit = Arc::clone(on_before_exit);
            builder = builder.on_before_exit(move || on_before_exit());
        }
        builder.build()
    }
}

impl<R: Runtime> UpdateFeed for PluginFeed<R> {
    type Release = Update;
    type Package = Vec<u8>;

    fn check(&self) -> Result<Option<Found<Update>>, CheckError> {
        let updater = self
            .updater()
            .map_err(|error| CheckError(error.to_string()))?;
        let update = tauri::async_runtime::block_on(updater.check())
            .map_err(|error| CheckError(error.to_string()))?;
        update
            .map(|update| {
                let version = Version::parse(&update.version)
                    .map_err(|error| CheckError(format!("invalid version: {error}")))?;
                Ok(Found {
                    version,
                    release: update,
                })
            })
            .transpose()
    }

    fn download(
        &self,
        release: &Update,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Vec<u8>, DownloadError> {
        let mut received = 0u64;
        let on_chunk = |chunk: usize, total: Option<u64>| {
            received += chunk as u64;
            progress(received, total);
        };
        tauri::async_runtime::block_on(release.download(on_chunk, || {})).map_err(|error| {
            if is_signature_error(&error) {
                DownloadError::Unverified(error.to_string())
            } else {
                DownloadError::Failed(error.to_string())
            }
        })
    }
}

fn is_signature_error(error: &Error) -> bool {
    matches!(
        error,
        Error::Minisign(_)
            | Error::Base64(_)
            | Error::SignedVersionMismatch { .. }
            | Error::MissingSignedVersion
    )
}

/// Runs the downloaded NSIS installer, which replaces Echo and starts it again.
/// On Windows the plugin exits Echo right after starting the installer.
pub struct PluginInstaller;

impl Installer<Update, Vec<u8>> for PluginInstaller {
    fn install(&self, release: &Update, package: Vec<u8>) -> Result<(), String> {
        if cfg!(debug_assertions) {
            // A development run must never replace the user's installed Echo.
            return Err(format!(
                "development build: not installing update {}",
                release.version
            ));
        }
        release.install(package).map_err(|error| error.to_string())
    }
}

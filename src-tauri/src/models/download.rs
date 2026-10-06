//! Downloading one Model file: resume through a byte range, size and SHA-256 verification,
//! timeouts, progress and cancellation (`models.md` rules 9–14).
//!
//! The transfer writes into the `.partial` file and renames it to the final name only after the
//! checksum matched (rule 4). Which failures keep the partial file and which delete it follows
//! the spec exactly; see [`DownloadError`].

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::StatusCode;
use reqwest::header::{CONTENT_RANGE, RANGE};
use sha2::{Digest, Sha256};
use tokio::sync::watch;
use tokio::time::{Instant as TokioInstant, sleep_until};

/// Timing rules of a download. The defaults are the spec's; tests shorten them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DownloadConfig {
    /// Connecting must succeed within this time (rule 11: 15 s).
    pub connect_timeout: Duration,
    /// With no data for this long the download fails as "stalled" (rule 11: 60 s).
    pub stall_timeout: Duration,
    /// Progress is reported no more often than this (rule 13: 100 ms).
    pub progress_min_interval: Duration,
    /// …and at least this often while transferring. The spec's ceiling is 250 ms; 200 ms leaves
    /// room for Windows timer granularity.
    pub progress_max_interval: Duration,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(15),
            stall_timeout: Duration::from_secs(60),
            progress_min_interval: Duration::from_millis(100),
            progress_max_interval: Duration::from_millis(200),
        }
    }
}

/// What to download and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTarget {
    pub url: String,
    /// The exact expected size in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256 of the whole file.
    pub sha256: String,
    pub partial: PathBuf,
    pub final_path: PathBuf,
}

/// A progress report.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    Transferring {
        downloaded: u64,
        total: u64,
        bytes_per_second: u64,
    },
    /// The last byte arrived; the checksum is being computed.
    Verifying,
}

/// Why a download did not finish. The comment on each variant says what happened to the partial
/// file.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    /// Connecting or transferring failed. Partial kept, so a retry resumes (rule 11).
    #[error("network error: {0}")]
    Network(String),
    /// No data arrived within the stall timeout. Partial kept (rule 11).
    #[error("the download stalled")]
    Stalled,
    /// The server resumed at the wrong byte or refused the range. Partial deleted, so the next
    /// attempt starts from zero (rule 9).
    #[error("the server could not resume the download: {0}")]
    BadRange(String),
    /// The server announced or sent a different total size. Partial deleted (rule 10).
    #[error("the server sent {actual} bytes instead of {expected}")]
    SizeMismatch { expected: u64, actual: u64 },
    /// The checksum did not match. File deleted (rule 12).
    #[error("the downloaded file is corrupted")]
    Corrupted,
    /// Reading or writing the file failed. Partial kept.
    #[error("could not write the Model file: {0}")]
    Storage(String),
    /// The user cancelled. Partial kept, so the download can be resumed (rule 14).
    #[error("cancelled")]
    Cancelled,
}

impl From<io::Error> for DownloadError {
    fn from(error: io::Error) -> Self {
        DownloadError::Storage(error.to_string())
    }
}

/// Stops a running download. Clones control the same download.
#[derive(Debug, Clone)]
pub struct CancelToken {
    tx: Arc<watch::Sender<bool>>,
}

impl Default for CancelToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancelToken {
    pub fn new() -> Self {
        Self {
            tx: Arc::new(watch::channel(false).0),
        }
    }

    pub fn cancel(&self) {
        self.tx.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.tx.borrow()
    }

    async fn cancelled(&self) {
        let mut rx = self.tx.subscribe();
        // The sender lives in `self`, so `wait_for` only returns once cancelled.
        let _ = rx.wait_for(|cancelled| *cancelled).await;
    }
}

/// The HTTP client used for Model downloads, with the connect timeout of rule 11.
pub fn http_client(config: &DownloadConfig) -> reqwest::Result<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(config.connect_timeout)
        .user_agent(concat!("Echo/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// Downloads `target`, resuming a partial file, and verifies it. On success the final file
/// exists and the partial file is gone.
pub async fn download(
    client: &reqwest::Client,
    target: &DownloadTarget,
    config: &DownloadConfig,
    cancel: &CancelToken,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<(), DownloadError> {
    if cancel.is_cancelled() {
        return Err(DownloadError::Cancelled);
    }
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(DownloadError::Cancelled),
        result = run(client, target, config, on_progress) => result,
    }
}

async fn run(
    client: &reqwest::Client,
    target: &DownloadTarget,
    config: &DownloadConfig,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<(), DownloadError> {
    if let Some(parent) = target.partial.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut have = fs::metadata(&target.partial).map_or(0, |m| m.len());
    if have > target.size {
        log::info!("partial download larger than expected; starting over");
        remove_partial(target);
        have = 0;
    }
    if have < target.size {
        transfer(client, target, config, have, on_progress).await?;
    }
    on_progress(Progress::Verifying);
    verify(target).await
}

/// Requests the bytes from `have` onwards and appends them to the partial file.
async fn transfer(
    client: &reqwest::Client,
    target: &DownloadTarget,
    config: &DownloadConfig,
    have: u64,
    on_progress: &mut (dyn FnMut(Progress) + Send),
) -> Result<(), DownloadError> {
    let mut request = client.get(&target.url);
    if have > 0 {
        request = request.header(RANGE, format!("bytes={have}-"));
    }
    let mut response = match tokio::time::timeout(config.stall_timeout, request.send()).await {
        Err(_) => return Err(DownloadError::Stalled),
        Ok(Err(error)) => return Err(network_error(&error)),
        Ok(Ok(response)) => response,
    };

    let start = match response.status() {
        StatusCode::OK => {
            // Either a fresh download or a server that ignored the range: start from zero.
            if let Some(length) = response.content_length()
                && length != target.size
            {
                remove_partial(target);
                return Err(DownloadError::SizeMismatch {
                    expected: target.size,
                    actual: length,
                });
            }
            if have > 0 {
                log::info!("server ignored the range request; restarting from zero");
            }
            0
        }
        StatusCode::PARTIAL_CONTENT => {
            let range = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_content_range);
            let Some((start, total)) = range else {
                remove_partial(target);
                return Err(DownloadError::BadRange(
                    "missing or invalid Content-Range".into(),
                ));
            };
            if start != have {
                remove_partial(target);
                return Err(DownloadError::BadRange(format!(
                    "the server resumed at byte {start} instead of {have}"
                )));
            }
            if let Some(total) = total
                && total != target.size
            {
                remove_partial(target);
                return Err(DownloadError::SizeMismatch {
                    expected: target.size,
                    actual: total,
                });
            }
            start
        }
        StatusCode::RANGE_NOT_SATISFIABLE => {
            remove_partial(target);
            return Err(DownloadError::BadRange(
                "the server reported the range as invalid".into(),
            ));
        }
        status => {
            return Err(DownloadError::Network(format!(
                "the server answered HTTP {}",
                status.as_u16()
            )));
        }
    };

    let mut file = if start == 0 {
        File::create(&target.partial)?
    } else {
        OpenOptions::new().append(true).open(&target.partial)?
    };

    let mut downloaded = start;
    let mut speed = SpeedMeter::new(Instant::now(), downloaded);
    let report = |downloaded: u64, speed: &mut SpeedMeter| Progress::Transferring {
        downloaded,
        total: target.size,
        bytes_per_second: speed.rate(Instant::now(), downloaded),
    };
    on_progress(report(downloaded, &mut speed));
    let mut last_report = TokioInstant::now();
    let mut last_data = TokioInstant::now();

    loop {
        tokio::select! {
            chunk = response.chunk() => match chunk {
                Err(error) => {
                    file.flush()?;
                    return Err(network_error(&error));
                }
                Ok(None) => break,
                Ok(Some(bytes)) => {
                    let received = downloaded + bytes.len() as u64;
                    if received > target.size {
                        drop(file);
                        remove_partial(target);
                        return Err(DownloadError::SizeMismatch {
                            expected: target.size,
                            actual: received,
                        });
                    }
                    file.write_all(&bytes)?;
                    downloaded = received;
                    last_data = TokioInstant::now();
                    if last_data - last_report >= config.progress_min_interval {
                        on_progress(report(downloaded, &mut speed));
                        last_report = last_data;
                    }
                }
            },
            () = sleep_until(last_report + config.progress_max_interval) => {
                on_progress(report(downloaded, &mut speed));
                last_report = TokioInstant::now();
            }
            () = sleep_until(last_data + config.stall_timeout) => {
                file.flush()?;
                return Err(DownloadError::Stalled);
            }
        }
    }
    file.sync_all()?;
    drop(file);

    if downloaded < target.size {
        return Err(DownloadError::Network(
            "the connection closed before the download finished".into(),
        ));
    }
    Ok(())
}

/// Hashes the complete partial file and, when it matches, renames it to its final name.
async fn verify(target: &DownloadTarget) -> Result<(), DownloadError> {
    let path = target.partial.clone();
    let actual = tokio::task::spawn_blocking(move || sha256_file(&path))
        .await
        .map_err(|e| DownloadError::Storage(e.to_string()))??;
    if !actual.eq_ignore_ascii_case(&target.sha256) {
        log::warn!("checksum mismatch for {}", target.final_path.display());
        remove_partial(target);
        return Err(DownloadError::Corrupted);
    }
    fs::rename(&target.partial, &target.final_path)?;
    Ok(())
}

/// The lower-case hex SHA-256 of a file.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn remove_partial(target: &DownloadTarget) {
    if let Err(error) = fs::remove_file(&target.partial)
        && error.kind() != io::ErrorKind::NotFound
    {
        log::warn!("could not delete {}: {error}", target.partial.display());
    }
}

/// Parses `bytes <start>-<end>/<total>` into the start and the total (`None` when `*`).
fn parse_content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim_start();
    let (range, total) = rest.split_once('/')?;
    let (start, _end) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let total = match total.trim() {
        "*" => None,
        total => Some(total.parse().ok()?),
    };
    Some((start, total))
}

/// A plain-language reason for a transport failure.
fn network_error(error: &reqwest::Error) -> DownloadError {
    let reason = if error.is_connect() {
        if error.is_timeout() {
            "could not connect to the server in time".to_owned()
        } else {
            "could not connect to the server".to_owned()
        }
    } else if error.is_timeout() {
        return DownloadError::Stalled;
    } else {
        let mut reason = error.to_string();
        let mut source = std::error::Error::source(error);
        while let Some(inner) = source {
            reason = format!("{reason}: {inner}");
            source = inner.source();
        }
        reason
    };
    DownloadError::Network(reason)
}

/// The transfer rate over the last two seconds.
struct SpeedMeter {
    samples: VecDeque<(Instant, u64)>,
}

impl SpeedMeter {
    const WINDOW: Duration = Duration::from_secs(2);

    fn new(now: Instant, bytes: u64) -> Self {
        Self {
            samples: VecDeque::from([(now, bytes)]),
        }
    }

    fn rate(&mut self, now: Instant, bytes: u64) -> u64 {
        self.samples.push_back((now, bytes));
        while self.samples.len() > 2 && now.duration_since(self.samples[1].0) >= Self::WINDOW {
            self.samples.pop_front();
        }
        let (then, before) = self.samples[0];
        let elapsed = now.duration_since(then).as_secs_f64();
        if elapsed <= 0.0 {
            0
        } else {
            ((bytes - before) as f64 / elapsed) as u64
        }
    }
}

#[cfg(test)]
mod tests;

//! `models.md` acceptance tests 1–10 and 12 against the local test server.

use std::sync::Mutex;

use super::*;
use crate::models::test_server::{Behaviour, TestServer, sha256_hex, test_bytes};

const SIZE: usize = 200_000;

struct Case {
    _dir: tempfile::TempDir,
    server: TestServer,
    target: DownloadTarget,
    body: Vec<u8>,
}

fn case(behaviour: impl FnOnce(&mut Behaviour)) -> Case {
    let body = test_bytes(SIZE);
    let mut b = Behaviour::serving(body.clone());
    behaviour(&mut b);
    let server = TestServer::start(b);
    let dir = tempfile::tempdir().unwrap();
    let target = DownloadTarget {
        url: server.url(),
        size: SIZE as u64,
        sha256: sha256_hex(&body),
        partial: dir.path().join("models").join("m.gguf.partial"),
        final_path: dir.path().join("models").join("m.gguf"),
    };
    Case {
        _dir: dir,
        server,
        target,
        body,
    }
}

fn fast() -> DownloadConfig {
    DownloadConfig {
        connect_timeout: Duration::from_secs(5),
        stall_timeout: Duration::from_secs(5),
        ..DownloadConfig::default()
    }
}

fn run_download(case: &Case, config: DownloadConfig) -> (Result<(), DownloadError>, Vec<Progress>) {
    run_with(case, config, &CancelToken::new(), |_| {})
}

fn run_with(
    case: &Case,
    config: DownloadConfig,
    cancel: &CancelToken,
    mut also: impl FnMut(&Progress) + Send,
) -> (Result<(), DownloadError>, Vec<Progress>) {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    let client = http_client(&config).unwrap();
    let events = Mutex::new(Vec::new());
    let result = runtime.block_on(download(&client, &case.target, &config, cancel, &mut |p| {
        also(&p);
        events.lock().unwrap().push(p);
    }));
    (result, events.into_inner().unwrap())
}

fn write_partial(case: &Case, bytes: &[u8]) {
    fs::create_dir_all(case.target.partial.parent().unwrap()).unwrap();
    fs::write(&case.target.partial, bytes).unwrap();
}

fn assert_verified(case: &Case) {
    assert_eq!(fs::read(&case.target.final_path).unwrap(), case.body);
    assert!(!case.target.partial.exists(), "no .partial left behind");
}

// Acceptance test 1.
#[test]
fn a_fresh_download_is_verified_and_renamed() {
    let case = case(|_| {});

    let (result, events) = run_download(&case, fast());

    result.unwrap();
    assert_verified(&case);
    assert_eq!(case.server.received().len(), 1);
    assert_eq!(case.server.received()[0].path, "/model.gguf");
    assert_eq!(case.server.received()[0].range, None);
    assert_eq!(events.last(), Some(&Progress::Verifying));
}

// Acceptance test 2.
#[test]
fn a_stopped_download_resumes_with_a_range_request() {
    let case = case(|b| b.close_after = Some(SIZE * 4 / 10));

    let (first, _) = run_download(&case, fast());
    assert!(matches!(first, Err(DownloadError::Network(_))), "{first:?}");
    let kept = fs::metadata(&case.target.partial).unwrap().len();
    assert_eq!(kept, (SIZE * 4 / 10) as u64, "the partial file is kept");

    case.server.behaviour().close_after = None;
    let (second, _) = run_download(&case, fast());

    second.unwrap();
    assert_verified(&case);
    assert_eq!(
        case.server.received()[1].range.as_deref(),
        Some(format!("bytes={kept}-").as_str())
    );
}

// Acceptance test 3.
#[test]
fn a_server_ignoring_the_range_restarts_from_zero() {
    let case = case(|b| b.honour_range = false);
    write_partial(&case, &case.body[..50_000]);

    let (result, _) = run_download(&case, fast());

    result.unwrap();
    assert_verified(&case);
    assert!(
        case.server.received()[0].range.is_some(),
        "a range was asked for"
    );
}

#[test]
fn a_server_ignoring_the_range_discards_even_a_wrong_partial() {
    let case = case(|b| b.honour_range = false);
    write_partial(&case, &[0xAA; 50_000]);

    run_download(&case, fast()).0.unwrap();

    assert_verified(&case);
}

// Acceptance test 4.
#[test]
fn a_range_starting_elsewhere_deletes_the_partial_and_fails() {
    let case = case(|b| b.range_start_offset = 1000);
    write_partial(&case, &case.body[..50_000]);

    let (result, _) = run_download(&case, fast());

    assert!(
        matches!(result, Err(DownloadError::BadRange(_))),
        "{result:?}"
    );
    assert!(!case.target.partial.exists());
    assert!(!case.target.final_path.exists());

    // The next attempt starts from zero.
    case.server.behaviour().range_start_offset = 0;
    run_download(&case, fast()).0.unwrap();
    assert_eq!(case.server.received()[1].range, None);
    assert_verified(&case);
}

#[test]
fn a_refused_range_deletes_the_partial_and_fails() {
    let case = case(|b| b.refuse_range = true);
    write_partial(&case, &case.body[..50_000]);

    let (result, _) = run_download(&case, fast());

    assert!(
        matches!(result, Err(DownloadError::BadRange(_))),
        "{result:?}"
    );
    assert!(!case.target.partial.exists());
}

// Acceptance test 5.
#[test]
fn an_oversized_partial_is_discarded() {
    let case = case(|_| {});
    write_partial(&case, &vec![1u8; SIZE + 10]);

    run_download(&case, fast()).0.unwrap();

    assert_verified(&case);
    assert_eq!(case.server.received()[0].range, None, "a fresh download");
}

// Acceptance test 6.
#[test]
fn a_full_size_partial_is_only_verified() {
    let case = case(|_| {});
    write_partial(&case, &case.body);

    let (result, events) = run_download(&case, fast());

    result.unwrap();
    assert_verified(&case);
    assert!(case.server.received().is_empty(), "no request was made");
    assert_eq!(events, vec![Progress::Verifying]);
}

// Acceptance test 7.
#[test]
fn wrong_bytes_of_the_right_length_are_deleted_as_corrupted() {
    let case = case(|b| b.body[1234] ^= 0xFF);

    let (result, _) = run_download(&case, fast());

    assert_eq!(result, Err(DownloadError::Corrupted));
    assert!(!case.target.partial.exists());
    assert!(!case.target.final_path.exists());
}

// Acceptance test 8.
#[test]
fn an_announced_size_mismatch_fails_and_deletes_the_partial() {
    let case = case(|b| b.advertised_size = Some(SIZE as u64 + 1));

    let (result, _) = run_download(&case, fast());

    assert_eq!(
        result,
        Err(DownloadError::SizeMismatch {
            expected: SIZE as u64,
            actual: SIZE as u64 + 1
        })
    );
    assert!(!case.target.partial.exists());
}

#[test]
fn a_resumed_range_with_a_different_total_fails_and_deletes_the_partial() {
    let case = case(|b| b.advertised_size = Some(999_999));
    write_partial(&case, &case.body[..50_000]);

    let (result, _) = run_download(&case, fast());

    assert!(
        matches!(result, Err(DownloadError::SizeMismatch { .. })),
        "{result:?}"
    );
    assert!(!case.target.partial.exists());
}

#[test]
fn more_bytes_than_expected_fail_and_delete_the_partial() {
    // No announced size, so only the bytes themselves show the mismatch.
    let case = case(|b| {
        b.body.extend_from_slice(&[0; 5000]);
        b.omit_length = true;
    });

    let (result, _) = run_download(&case, fast());

    assert!(
        matches!(result, Err(DownloadError::SizeMismatch { .. })),
        "{result:?}"
    );
    assert!(!case.target.partial.exists());
}

// Acceptance test 9.
#[test]
fn a_stalled_server_fails_as_stalled_and_keeps_the_partial() {
    let case = case(|b| b.stall_after = Some(SIZE / 2));
    let config = DownloadConfig {
        stall_timeout: Duration::from_millis(600),
        ..fast()
    };

    let started = Instant::now();
    let (result, _) = run_download(&case, config);

    assert_eq!(result, Err(DownloadError::Stalled));
    assert!(started.elapsed() < Duration::from_secs(4));
    assert_eq!(
        fs::metadata(&case.target.partial).unwrap().len(),
        (SIZE / 2) as u64
    );
}

// Acceptance test 10.
#[test]
fn cancel_stops_within_a_second_keeps_the_partial_and_resume_continues() {
    let case = case(|b| {
        b.chunk = 4000;
        b.chunk_delay = Duration::from_millis(20);
    });
    let cancel = CancelToken::new();
    let cancelled_at = Mutex::new(None);

    let (result, _) = run_with(&case, fast(), &cancel, |p| {
        if let Progress::Transferring { downloaded, .. } = p
            && *downloaded >= (SIZE * 3 / 10) as u64
            && !cancel.is_cancelled()
        {
            *cancelled_at.lock().unwrap() = Some(Instant::now());
            cancel.cancel();
        }
    });

    assert_eq!(result, Err(DownloadError::Cancelled));
    let cancelled_at = cancelled_at
        .into_inner()
        .unwrap()
        .expect("cancelled at 30%");
    assert!(cancelled_at.elapsed() < Duration::from_secs(1));
    let kept = fs::metadata(&case.target.partial).unwrap().len();
    assert!(
        kept >= (SIZE * 3 / 10) as u64 && kept < SIZE as u64,
        "{kept}"
    );

    case.server.behaviour().chunk_delay = Duration::ZERO;
    run_download(&case, fast()).0.unwrap();
    assert_eq!(
        case.server.received()[1].range.as_deref(),
        Some(format!("bytes={kept}-").as_str())
    );
    assert_verified(&case);
}

// Acceptance test 12.
#[test]
fn progress_arrives_between_every_100_and_250_ms() {
    // ~1.5 s of transfer in small, frequent pieces: many more chunks than reports.
    let case = case(|b| {
        b.chunk = 2000;
        b.chunk_delay = Duration::from_millis(15);
    });
    let stamps = Mutex::new(Vec::new());

    let (result, _) = run_with(&case, fast(), &CancelToken::new(), |p| {
        if matches!(p, Progress::Transferring { .. }) {
            stamps.lock().unwrap().push(Instant::now());
        }
    });

    result.unwrap();
    let stamps = stamps.into_inner().unwrap();
    assert!(stamps.len() >= 5, "{} reports", stamps.len());
    for pair in stamps.windows(2) {
        let gap = pair[1] - pair[0];
        assert!(gap >= Duration::from_millis(100), "too often: {gap:?}");
        assert!(gap <= Duration::from_millis(250), "too rare: {gap:?}");
    }
}

#[test]
fn progress_reports_bytes_total_and_speed() {
    let case = case(|b| {
        b.chunk = 4000;
        b.chunk_delay = Duration::from_millis(10);
    });

    let (result, events) = run_download(&case, fast());

    result.unwrap();
    let transferring: Vec<_> = events
        .iter()
        .filter_map(|p| match p {
            Progress::Transferring {
                downloaded,
                total,
                bytes_per_second,
            } => Some((*downloaded, *total, *bytes_per_second)),
            Progress::Verifying => None,
        })
        .collect();
    assert!(
        transferring
            .iter()
            .all(|(_, total, _)| *total == SIZE as u64)
    );
    assert!(transferring.windows(2).all(|w| w[0].0 <= w[1].0));
    assert!(transferring.iter().skip(1).any(|(_, _, speed)| *speed > 0));
}

#[test]
fn a_connection_failure_keeps_the_partial_for_a_retry() {
    let mut case = case(|_| {});
    write_partial(&case, &case.body[..1000]);
    // A port nobody listens on.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    case.target.url = format!("http://{}/x", closed.local_addr().unwrap());
    drop(closed);

    let (result, _) = run_download(&case, fast());

    assert!(
        matches!(result, Err(DownloadError::Network(_))),
        "{result:?}"
    );
    assert_eq!(fs::metadata(&case.target.partial).unwrap().len(), 1000);
}

#[test]
fn content_range_is_parsed() {
    assert_eq!(
        parse_content_range("bytes 10-99/100"),
        Some((10, Some(100)))
    );
    assert_eq!(parse_content_range("bytes 10-99/*"), Some((10, None)));
    assert_eq!(parse_content_range("items 1-2/3"), None);
    assert_eq!(parse_content_range("bytes x-2/3"), None);
}

// Acceptance test 23: the real file from the real source (network, ~257 MB). Run with
// `cargo test --lib real_download -- --ignored --nocapture`.
#[test]
#[ignore = "downloads Whisper small (~257 MB) from Hugging Face"]
fn real_download_of_whisper_small_verifies_against_the_published_checksum() {
    use crate::models::{ModelId, ModelStorage};

    let dir = tempfile::tempdir().unwrap();
    let storage = ModelStorage::new(dir.path());
    let id = ModelId::WhisperSmall;
    let file = storage.file(id).clone();
    let target = DownloadTarget {
        url: file.url,
        size: file.size,
        sha256: file.sha256,
        partial: storage.partial_path(id),
        final_path: storage.final_path(id),
    };
    let config = DownloadConfig::default();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let client = http_client(&config).unwrap();
    let started = Instant::now();
    let mut last = 0;

    runtime
        .block_on(download(
            &client,
            &target,
            &config,
            &CancelToken::new(),
            &mut |p| {
                if let Progress::Transferring {
                    downloaded,
                    bytes_per_second,
                    ..
                } = p
                    && downloaded / 50_000_000 != last
                {
                    last = downloaded / 50_000_000;
                    eprintln!("{downloaded} bytes, {} MB/s", bytes_per_second / 1_048_576);
                }
            },
        ))
        .unwrap();

    eprintln!("downloaded and verified in {:?}", started.elapsed());
    assert!(storage.is_downloaded(id));
}

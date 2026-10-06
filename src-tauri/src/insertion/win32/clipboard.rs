//! [`Clipboard`] on Windows.
//!
//! A hidden message-only window on its own thread owns Echo's clipboard contents. The Transcript
//! is offered with *delayed rendering*: Windows asks that window for the text
//! (`WM_RENDERFORMAT`) the moment an application reads it, which is how Echo observes the read
//! (rule 34.4). Ownership tells whether anyone else changed the clipboard since: writing to the
//! clipboard always takes ownership away from Echo's window (rule 34.5).
//!
//! Every clipboard operation runs on that thread, because the window must keep pumping
//! messages to answer render requests and the clipboard must be closed by the thread that
//! opened it.

use std::cell::{Cell, RefCell};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetEnhMetaFileBits, HENHMETAFILE, SetEnhMetaFileBits};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardOwner,
    OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{
    GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock,
};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE,
    MSG, PostThreadMessageW, RegisterClassW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
    WM_DESTROYCLIPBOARD, WM_QUIT, WM_RENDERALLFORMATS, WM_RENDERFORMAT, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

use super::super::paste::{Clipboard, Restore};

/// Thread message: run the queued jobs.
const WM_RUN_JOBS: u32 = WM_APP + 1;
/// Another process may hold the clipboard open for a moment; try this often before giving up.
const OPEN_ATTEMPTS: u32 = 10;
const OPEN_RETRY: Duration = Duration::from_millis(10);

/// Standard formats whose data is not a memory block, or which Windows synthesises from a format
/// that is saved (`CF_BITMAP` from `CF_DIB`, `CF_METAFILEPICT` from `CF_ENHMETAFILE`).
const SKIPPED_FORMATS: [u32; 6] = [
    2,    // CF_BITMAP
    3,    // CF_METAFILEPICT
    9,    // CF_PALETTE
    0x80, // CF_OWNERDISPLAY
    0x82, // CF_DSPBITMAP
    0x83, // CF_DSPMETAFILEPICT
];
/// `CF_PRIVATEFIRST..=CF_GDIOBJLAST`: handles only their owner understands.
const PRIVATE_FORMATS: std::ops::RangeInclusive<u32> = 0x200..=0x3FF;
/// `CF_ENHMETAFILE` and `CF_DSPENHMETAFILE`, whose data is a metafile handle.
const METAFILE_FORMATS: [u32; 2] = [14, 0x8E];

/// The clipboard contents saved before a paste, in every readable format.
#[derive(Debug, Default)]
pub struct Saved {
    items: Vec<SavedFormat>,
}

#[derive(Debug)]
struct SavedFormat {
    format: u32,
    data: Data,
}

#[derive(Debug)]
enum Data {
    Memory(Vec<u8>),
    EnhancedMetafile(Vec<u8>),
}

/// State the clipboard thread shares with the restore.
#[derive(Default)]
struct Shared {
    window: AtomicIsize,
    last_read: Mutex<Option<Instant>>,
    /// Someone else emptied the clipboard since Echo's last put.
    lost: AtomicBool,
}

impl Shared {
    fn window(&self) -> HWND {
        HWND(self.window.load(Ordering::SeqCst) as *mut _)
    }

    fn set_last_read(&self, read: Option<Instant>) {
        *self
            .last_read
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = read;
    }
}

type Job = Box<dyn FnOnce(HWND) + Send>;

thread_local! {
    /// The Transcript (UTF-16, NUL-terminated) waiting to be rendered on request.
    static PENDING: RefCell<Option<Vec<u16>>> = const { RefCell::new(None) };
    /// Set while Echo empties the clipboard itself, so that is not mistaken for a change.
    static SELF_EMPTYING: Cell<bool> = const { Cell::new(false) };
    static SHARED: RefCell<Option<Arc<Shared>>> = const { RefCell::new(None) };
}

/// The Windows clipboard, operated from Echo's clipboard thread.
pub struct WindowsClipboard {
    jobs: mpsc::Sender<Job>,
    thread_id: u32,
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl WindowsClipboard {
    /// Starts the clipboard thread and its window.
    pub fn start() -> Result<Self, String> {
        let shared = Arc::new(Shared::default());
        let (jobs, job_queue) = mpsc::channel::<Job>();
        let (ready, started) = mpsc::channel::<Result<u32, String>>();
        let thread_shared = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("echo-clipboard".into())
            .spawn(move || run(thread_shared, job_queue, ready))
            .map_err(|e| format!("could not start the clipboard thread: {e}"))?;
        let thread_id = started
            .recv()
            .map_err(|_| "the clipboard thread stopped".to_owned())??;
        Ok(Self {
            jobs,
            thread_id,
            shared,
            thread: Mutex::new(Some(thread)),
        })
    }

    /// Runs `job` on the clipboard thread and waits for its result.
    fn call<R: Send + 'static>(
        &self,
        job: impl FnOnce(HWND, &Shared) -> R + Send + 'static,
    ) -> Result<R, String> {
        let (reply, result) = mpsc::sync_channel(1);
        let shared = Arc::clone(&self.shared);
        self.jobs
            .send(Box::new(move |window| {
                let _ = reply.send(job(window, &shared));
            }))
            .map_err(|_| "the clipboard thread stopped".to_owned())?;
        // SAFETY: posting a message to a thread has no memory-safety preconditions.
        unsafe { PostThreadMessageW(self.thread_id, WM_RUN_JOBS, WPARAM(0), LPARAM(0)) }
            .map_err(|e| format!("could not reach the clipboard thread: {e}"))?;
        result
            .recv()
            .map_err(|_| "the clipboard thread stopped".to_owned())
    }
}

impl Drop for WindowsClipboard {
    fn drop(&mut self) {
        // SAFETY: as in `call`.
        let _ = unsafe { PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) };
        let thread = self
            .thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Clipboard for WindowsClipboard {
    type Saved = Saved;

    fn save(&self) -> Result<Saved, String> {
        self.call(|window, _| {
            let _open = OpenGuard::open(window)?;
            Ok(save_open_clipboard())
        })?
    }

    fn put(&self, text: &str) -> Result<(), String> {
        let mut units: Vec<u16> = text.encode_utf16().collect();
        units.push(0);
        self.call(move |window, shared| {
            let _open = OpenGuard::open(window)?;
            empty_own_clipboard()?;
            shared.lost.store(false, Ordering::SeqCst);
            shared.set_last_read(None);
            PENDING.set(Some(units));
            set_delayed(CF_UNICODETEXT.0.into())?;
            mark_excluded(&[], true)
        })?
    }

    fn last_read(&self) -> Option<Instant> {
        *self
            .shared
            .last_read
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn changed(&self) -> bool {
        // SAFETY: GetClipboardOwner has no preconditions.
        let owner = unsafe { GetClipboardOwner() }.ok();
        self.shared.lost.load(Ordering::SeqCst) || owner != Some(self.shared.window())
    }

    fn restore(&self, saved: Saved) -> Result<Restore, String> {
        self.call(move |window, shared| {
            let _open = OpenGuard::open(window)?;
            // Checked while the clipboard is open, so nobody can write in between.
            // SAFETY: GetClipboardOwner has no preconditions.
            let owner = unsafe { GetClipboardOwner() }.ok();
            if shared.lost.load(Ordering::SeqCst) || owner != Some(window) {
                return Ok(Restore::KeptNewer);
            }
            empty_own_clipboard()?;
            for item in &saved.items {
                if let Err(error) = set_saved(item) {
                    log::warn!(
                        "could not restore clipboard format {}: {error}",
                        item.format
                    );
                }
            }
            // An empty clipboard stays empty, without even the history marks.
            if !saved.items.is_empty() {
                mark_excluded(&saved.items, false)?;
            }
            Ok(Restore::Restored)
        })?
    }
}

#[cfg(test)]
impl WindowsClipboard {
    /// Puts `saved` back whoever owns the clipboard — for tests protecting the user's clipboard.
    pub(crate) fn restore_unconditionally(&self, saved: Saved) -> Result<(), String> {
        self.call(move |window, _| {
            let _open = OpenGuard::open(window)?;
            empty_own_clipboard()?;
            for item in &saved.items {
                set_saved(item)?;
            }
            if !saved.items.is_empty() {
                mark_excluded(&saved.items, false)?;
            }
            Ok(())
        })?
    }
}

#[cfg(test)]
impl Saved {
    /// Format, size and a hash of each saved format, to compare contents without printing them.
    pub(crate) fn fingerprint(&self) -> Vec<(u32, usize, u64)> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        self.items
            .iter()
            .map(|item| {
                let bytes = match &item.data {
                    Data::Memory(b) | Data::EnhancedMetafile(b) => b,
                };
                let mut hasher = DefaultHasher::new();
                bytes.hash(&mut hasher);
                (item.format, bytes.len(), hasher.finish())
            })
            .collect()
    }
}

/// The clipboard thread: creates the owner window, then runs jobs and answers render requests
/// until told to quit.
fn run(shared: Arc<Shared>, jobs: mpsc::Receiver<Job>, ready: mpsc::Sender<Result<u32, String>>) {
    SHARED.set(Some(Arc::clone(&shared)));
    let window = match create_window() {
        Ok(window) => window,
        Err(error) => {
            let _ = ready.send(Err(error));
            return;
        }
    };
    shared.window.store(window.0 as isize, Ordering::SeqCst);
    // SAFETY: GetCurrentThreadId has no preconditions. The window gave this thread a message
    // queue, so thread messages posted from now on arrive.
    let _ = ready.send(Ok(unsafe { GetCurrentThreadId() }));

    let mut msg = MSG::default();
    // SAFETY: `msg` is a valid MSG to fill.
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        if msg.hwnd.is_invalid() && msg.message == WM_RUN_JOBS {
            while let Ok(job) = jobs.try_recv() {
                job(window);
            }
        } else {
            // SAFETY: `msg` came from GetMessageW.
            unsafe { DispatchMessageW(&msg) };
        }
    }
    // Windows asks the window to render a still-pending Transcript before it is destroyed.
    // SAFETY: the window belongs to this thread.
    let _ = unsafe { DestroyWindow(window) };
}

fn create_window() -> Result<HWND, String> {
    let class = w!("EchoClipboardOwner");
    // SAFETY: plain Win32 calls with valid arguments; the class name is a static string.
    unsafe {
        let instance = GetModuleHandleW(None).map_err(|e| e.to_string())?;
        let class_info = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance.into(),
            lpszClassName: class,
            ..Default::default()
        };
        // Fails harmlessly when the class is already registered by an earlier clipboard.
        RegisterClassW(&class_info);
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class,
            w!("Echo clipboard"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(instance.into()),
            None,
        )
        .map_err(|e| format!("could not create the clipboard window: {e}"))
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_RENDERFORMAT => {
            if wparam.0 == usize::from(CF_UNICODETEXT.0) && render_pending() {
                SHARED.with_borrow(|shared| {
                    if let Some(shared) = shared {
                        shared.set_last_read(Some(Instant::now()));
                    }
                });
            }
            LRESULT(0)
        }
        WM_RENDERALLFORMATS => {
            // The window is going away; leave the Transcript on the clipboard as real data.
            // SAFETY: called on the window's own thread while it still exists.
            unsafe {
                if OpenClipboard(Some(window)).is_ok() {
                    if GetClipboardOwner().ok() == Some(window) {
                        render_pending();
                    }
                    let _ = CloseClipboard();
                }
            }
            LRESULT(0)
        }
        WM_DESTROYCLIPBOARD => {
            PENDING.set(None);
            if !SELF_EMPTYING.get() {
                SHARED.with_borrow(|shared| {
                    if let Some(shared) = shared {
                        shared.lost.store(true, Ordering::SeqCst);
                    }
                });
            }
            LRESULT(0)
        }
        // SAFETY: forwarding the arguments Windows gave us.
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

/// Hands the pending Transcript to Windows in answer to a render request. Must run while the
/// clipboard is open for rendering (inside `WM_RENDERFORMAT`, or opened by us).
fn render_pending() -> bool {
    let Some(units) = PENDING.with_borrow(Clone::clone) else {
        return false;
    };
    let bytes: Vec<u8> = units.iter().flat_map(|u| u.to_ne_bytes()).collect();
    set_memory(CF_UNICODETEXT.0.into(), &bytes).is_ok()
}

/// Keeps the clipboard open for its lifetime.
struct OpenGuard;

impl OpenGuard {
    fn open(window: HWND) -> Result<Self, String> {
        let mut last_error = String::new();
        for attempt in 0..OPEN_ATTEMPTS {
            if attempt > 0 {
                std::thread::sleep(OPEN_RETRY);
            }
            // SAFETY: `window` is the clipboard thread's own window.
            match unsafe { OpenClipboard(Some(window)) } {
                Ok(()) => return Ok(Self),
                Err(error) => last_error = error.to_string(),
            }
        }
        Err(format!("could not open the clipboard: {last_error}"))
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        // SAFETY: the clipboard was opened by this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Empties the open clipboard, making Echo's window its owner.
fn empty_own_clipboard() -> Result<(), String> {
    SELF_EMPTYING.set(true);
    // SAFETY: the clipboard is open on this thread.
    let result = unsafe { EmptyClipboard() };
    SELF_EMPTYING.set(false);
    PENDING.set(None);
    result.map_err(|e| format!("could not empty the clipboard: {e}"))
}

fn save_open_clipboard() -> Saved {
    let mut items = Vec::new();
    let mut format = 0;
    loop {
        // SAFETY: the clipboard is open on this thread.
        format = unsafe { EnumClipboardFormats(format) };
        if format == 0 {
            break;
        }
        if SKIPPED_FORMATS.contains(&format) || PRIVATE_FORMATS.contains(&format) {
            continue;
        }
        // Unreadable formats (e.g. the owner failed to render) are skipped (rule 34.1).
        // SAFETY: the clipboard is open on this thread.
        let Ok(handle) = (unsafe { GetClipboardData(format) }) else {
            continue;
        };
        let data = if METAFILE_FORMATS.contains(&format) {
            read_metafile(HENHMETAFILE(handle.0)).map(Data::EnhancedMetafile)
        } else {
            read_memory(HGLOBAL(handle.0)).map(Data::Memory)
        };
        if let Some(data) = data {
            items.push(SavedFormat { format, data });
        }
    }
    Saved { items }
}

fn read_memory(memory: HGLOBAL) -> Option<Vec<u8>> {
    // SAFETY: `memory` is clipboard data owned by the system; it is only read, between
    // GlobalLock and GlobalUnlock, for at most GlobalSize bytes.
    unsafe {
        let size = GlobalSize(memory);
        if size == 0 {
            return None;
        }
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            return None;
        }
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), size).to_vec();
        let _ = GlobalUnlock(memory);
        Some(bytes)
    }
}

fn read_metafile(metafile: HENHMETAFILE) -> Option<Vec<u8>> {
    // SAFETY: `metafile` is clipboard data owned by the system; it is only copied out.
    unsafe {
        let size = GetEnhMetaFileBits(metafile, None);
        if size == 0 {
            return None;
        }
        let mut bytes = vec![0; size as usize];
        (GetEnhMetaFileBits(metafile, Some(&mut bytes)) == size).then_some(bytes)
    }
}

fn set_saved(item: &SavedFormat) -> Result<(), String> {
    match &item.data {
        Data::Memory(bytes) => set_memory(item.format, bytes),
        Data::EnhancedMetafile(bytes) => {
            // SAFETY: `bytes` came from GetEnhMetaFileBits. The clipboard owns the handle once
            // SetClipboardData succeeds.
            unsafe {
                let metafile = SetEnhMetaFileBits(bytes);
                if metafile.is_invalid() {
                    return Err("invalid metafile".into());
                }
                SetClipboardData(item.format, Some(HANDLE(metafile.0)))
                    .map(drop)
                    .map_err(|e| e.to_string())
            }
        }
    }
}

/// Puts `bytes` on the open clipboard (or answers a render request) as `format`.
fn set_memory(format: u32, bytes: &[u8]) -> Result<(), String> {
    // SAFETY: the block is allocated with the requested size, written within it, and handed
    // to the clipboard (which then owns it) or freed if that fails.
    unsafe {
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1)).map_err(|e| e.to_string())?;
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err("could not lock clipboard memory".into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
        let _ = GlobalUnlock(memory);
        if let Err(error) = SetClipboardData(format, Some(HANDLE(memory.0))) {
            let _ = GlobalFree(Some(memory));
            return Err(error.to_string());
        }
        Ok(())
    }
}

/// Offers `format` with delayed rendering.
fn set_delayed(format: u32) -> Result<(), String> {
    // SAFETY: the clipboard is open on this thread and owned by its window.
    match unsafe { SetClipboardData(format, None) } {
        // A delayed format "succeeds" with a null handle, which the binding reports as an
        // error carrying no error code.
        Ok(_) => Ok(()),
        Err(error) if error.code().is_ok() => Ok(()),
        Err(error) => Err(format!("could not write the clipboard: {error}")),
    }
}

/// Marks the open clipboard's contents so Windows keeps them out of clipboard history and cloud
/// sync, and — for the Transcript — out of clipboard monitors. Marks already among `present`
/// (restored from the user's own contents) are kept as they were.
fn mark_excluded(present: &[SavedFormat], from_monitors: bool) -> Result<(), String> {
    let zero = 0u32.to_ne_bytes();
    let mut marks = vec![
        w!("CanIncludeInClipboardHistory"),
        w!("CanUploadToCloudClipboard"),
    ];
    if from_monitors {
        marks.push(w!("ExcludeClipboardContentFromMonitorProcessing"));
    }
    for name in marks {
        let format = register(name)?;
        if !present.iter().any(|item| item.format == format) {
            set_memory(format, &zero)?;
        }
    }
    Ok(())
}

fn register(name: PCWSTR) -> Result<u32, String> {
    // SAFETY: `name` is a static NUL-terminated string.
    match unsafe { RegisterClipboardFormatW(name) } {
        0 => Err("could not register a clipboard format".into()),
        format => Ok(format),
    }
}

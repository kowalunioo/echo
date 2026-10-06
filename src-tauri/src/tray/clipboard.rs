//! "Copy last Transcript" (rule 9): an ordinary copy, like Ctrl+C in any app — unlike the
//! Inserter's paste, the text stays on the clipboard and may enter clipboard history.

use windows::Win32::Foundation::{GlobalFree, HANDLE};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;

/// Puts `text` on the clipboard as Unicode text.
pub fn copy_text(text: &str) -> Result<(), String> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // Another application may hold the clipboard for a moment.
    let mut attempts = 0;
    // SAFETY: opening the clipboard has no memory-safety preconditions.
    while let Err(error) = unsafe { OpenClipboard(None) } {
        attempts += 1;
        if attempts == 10 {
            return Err(format!("the clipboard is busy: {error}"));
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let result = put(&wide);
    // SAFETY: the clipboard was opened above on this thread.
    let _ = unsafe { CloseClipboard() };
    result
}

fn put(wide: &[u16]) -> Result<(), String> {
    let bytes = std::mem::size_of_val(wide);
    // SAFETY: the clipboard is open on this thread; the block is allocated with the needed size,
    // written within it, and handed to the clipboard (which then owns it) or freed on failure.
    unsafe {
        EmptyClipboard().map_err(|e| e.to_string())?;
        let memory = GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| e.to_string())?;
        let pointer = GlobalLock(memory);
        if pointer.is_null() {
            let _ = GlobalFree(Some(memory));
            return Err("could not lock clipboard memory".into());
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), pointer.cast::<u16>(), wide.len());
        let _ = GlobalUnlock(memory);
        if let Err(error) = SetClipboardData(CF_UNICODETEXT.0.into(), Some(HANDLE(memory.0))) {
            let _ = GlobalFree(Some(memory));
            return Err(error.to_string());
        }
    }
    Ok(())
}

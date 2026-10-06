//! The Windows side of the real Inserter: the clipboard and keyboard seams of [`super::paste`]
//! implemented with the Win32 API.

mod clipboard;
mod desktop;

use std::sync::Arc;

pub use clipboard::WindowsClipboard;
pub use desktop::WindowsDesktop;

use super::InsertionError;
use super::paste::{PasteInserter, SystemClock};

/// The real Inserter on Windows.
pub type WindowsInserter = PasteInserter<WindowsClipboard, WindowsDesktop, SystemClock>;

/// Builds the real Inserter. It starts a small background thread that owns Echo's side of the
/// clipboard for as long as the Inserter lives.
pub fn system_inserter() -> Result<WindowsInserter, InsertionError> {
    let clipboard = WindowsClipboard::start().map_err(InsertionError::Failed)?;
    Ok(PasteInserter::new(
        Arc::new(clipboard),
        WindowsDesktop,
        Arc::new(SystemClock),
    ))
}

#[cfg(test)]
mod tests;

/// The real Inserter can be handed to other threads like any Inserter (`Inserter: Send`).
const _: fn() = || {
    fn assert_inserter<T: super::Inserter + 'static>() {}
    assert_inserter::<WindowsInserter>();
};

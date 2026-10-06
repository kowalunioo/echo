//! Free disk space, behind a trait so the "not enough space" rule is tested with a fake
//! (`models.md` rule 8, acceptance test 11).

use std::io;
use std::path::Path;

/// Reports how many bytes the current user may still write on the volume holding a path.
pub trait DiskSpace: Send + Sync {
    fn free_bytes(&self, path: &Path) -> io::Result<u64>;
}

/// The real probe (`GetDiskFreeSpaceExW`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemDiskSpace;

impl DiskSpace for SystemDiskSpace {
    fn free_bytes(&self, path: &Path) -> io::Result<u64> {
        // The models folder may not exist yet; ask about the nearest existing ancestor.
        let mut probe = path;
        while !probe.exists() {
            match probe.parent() {
                Some(parent) => probe = parent,
                None => break,
            }
        }
        free_bytes_for(probe)
    }
}

#[cfg(windows)]
fn free_bytes_for(path: &Path) -> io::Result<u64> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            directory: *const u16,
            free_to_caller: *mut u64,
            total: *mut u64,
            total_free: *mut u64,
        ) -> i32;
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free_to_caller = 0u64;
    // SAFETY: `wide` is a NUL-terminated UTF-16 path that outlives the call; the out pointer is
    // valid and the other two may be null.
    let ok = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free_to_caller,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(free_to_caller)
    }
}

#[cfg(not(windows))]
fn free_bytes_for(_path: &Path) -> io::Result<u64> {
    Ok(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_probe_answers_for_a_folder_that_does_not_exist_yet() {
        let dir = tempfile::tempdir().unwrap();
        let free = SystemDiskSpace
            .free_bytes(&dir.path().join("models").join("deeper"))
            .unwrap();
        assert!(free > 0);
    }
}

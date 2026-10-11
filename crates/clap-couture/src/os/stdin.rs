//! Stdin, read only when something is piped into it.

use std::io::{self, Read as _};

/// Everything piped into stdin; `None` when stdin is not a pipe, like a terminal, a file or
/// `/dev/null`. A socket counts as a pipe: some runtimes, like Node's, pipe a child's stdin
/// through one.
///
/// # Errors
/// Reading stdin failed.
pub(crate) fn read_piped() -> io::Result<Option<Vec<u8>>> {
    if !is_pipe() {
        return Ok(None);
    }
    let mut piped = Vec::new();
    io::stdin().lock().read_to_end(&mut piped)?;
    Ok(Some(piped))
}

#[cfg(unix)]
fn is_pipe() -> bool {
    use std::{
        fs::File,
        os::{fd::AsFd as _, unix::fs::FileTypeExt as _},
    };

    // A duplicate, so dropping the `File` leaves stdin open.
    let Ok(stdin) = io::stdin().as_fd().try_clone_to_owned() else { return false };
    File::from(stdin).metadata().is_ok_and(|metadata| {
        let file_type = metadata.file_type();
        file_type.is_fifo() || file_type.is_socket()
    })
}

#[cfg(windows)]
fn is_pipe() -> bool {
    use std::os::windows::io::AsRawHandle as _;

    use windows_sys::Win32::Storage::FileSystem::{FILE_TYPE_PIPE, GetFileType};

    // SAFETY: the handle is borrowed from stdin for the call. An invalid or missing one makes
    // `GetFileType` report `FILE_TYPE_UNKNOWN`.
    unsafe { GetFileType(io::stdin().as_raw_handle()) == FILE_TYPE_PIPE }
}

/// Elsewhere, there is no telling a pipe apart, so stdin is never read.
#[cfg(not(any(unix, windows)))]
const fn is_pipe() -> bool {
    false
}

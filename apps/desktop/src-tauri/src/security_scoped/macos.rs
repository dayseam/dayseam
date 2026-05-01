//! macOS implementation using `objc2-foundation` (`NSURL`, `NSData`, …).

use std::ffi::{CStr, OsStr};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{
    NSData, NSError, NSString, NSURLBookmarkCreationOptions, NSURLBookmarkResolutionOptions, NSURL,
};

use super::{ResolvedBookmark, SecurityScopedError};

pub struct SecurityScopedGuardInner {
    url: Retained<NSURL>,
    started: bool,
}

impl SecurityScopedGuardInner {
    /// Best-effort access for a filesystem path (e.g. tests and non-sandbox direct builds).
    /// For App Sandbox grants, prefer [`Self::from_bookmark`] so `startAccessing…` runs on the
    /// URL produced by bookmark resolution.
    pub fn new(path: &Path) -> Result<Self, SecurityScopedError> {
        let path_str = path.to_str().ok_or(SecurityScopedError::NonUtf8Path)?;
        let is_dir = path.metadata().map_err(SecurityScopedError::Io)?.is_dir();
        let ns_path = NSString::from_str(path_str);
        let url = NSURL::fileURLWithPath_isDirectory(&ns_path, is_dir);
        // SAFETY: `startAccessingSecurityScopedResource` is documented on file URLs from
        // security-scoped bookmarks or user-selected URLs; pairing with `stop` in `Drop`.
        let started = unsafe { url.startAccessingSecurityScopedResource() };
        if !started {
            return Err(SecurityScopedError::StartAccessDenied);
        }
        Ok(Self { url, started: true })
    }

    /// Recommended for MAS / sandbox: resolves bookmark bytes and starts access on that `NSURL`.
    pub fn from_bookmark(blob: &[u8]) -> Result<Self, SecurityScopedError> {
        let (url, _) = resolve_bookmark_url(blob)?;
        // SAFETY: Same pairing requirement as [`Self::new`].
        let started = unsafe { url.startAccessingSecurityScopedResource() };
        if !started {
            return Err(SecurityScopedError::StartAccessDenied);
        }
        Ok(Self { url, started: true })
    }

    pub fn stop(&mut self) {
        if self.started {
            unsafe {
                self.url.stopAccessingSecurityScopedResource();
            }
            self.started = false;
        }
    }
}

fn describe_ns_error(err: &NSError) -> String {
    err.localizedDescription().to_string()
}

fn resolve_bookmark_url(blob: &[u8]) -> Result<(Retained<NSURL>, bool), SecurityScopedError> {
    if blob.is_empty() {
        return Err(SecurityScopedError::EmptyBookmarkData);
    }

    let data = NSData::with_bytes(blob);
    let mut stale_flag = Bool::NO;
    // SAFETY: `stale_flag` is a valid `BOOL` out-parameter for Foundation.
    let url = unsafe {
        NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
            &data,
            NSURLBookmarkResolutionOptions::WithSecurityScope,
            None,
            &mut stale_flag,
        )
    }
    .map_err(|e| SecurityScopedError::Foundation(describe_ns_error(&e)))?;

    Ok((url, stale_flag.as_bool()))
}

fn path_buf_from_file_url(url: &NSURL) -> Result<PathBuf, SecurityScopedError> {
    let ptr = url.fileSystemRepresentation();
    // SAFETY: `fileSystemRepresentation` yields a NUL-terminated absolute path for file URLs.
    let cstr = unsafe { CStr::from_ptr(ptr.as_ptr()) };
    let bytes = cstr.to_bytes();
    Ok(PathBuf::from(OsStr::from_bytes(bytes)))
}

pub fn create_directory_bookmark(path: &Path) -> Result<Vec<u8>, SecurityScopedError> {
    let meta = path.metadata().map_err(SecurityScopedError::Io)?;
    if !meta.is_dir() {
        return Err(SecurityScopedError::NotADirectory);
    }

    let canonical = path.canonicalize().map_err(SecurityScopedError::Io)?;
    let path_str = canonical.to_str().ok_or(SecurityScopedError::NonUtf8Path)?;
    let ns_path = NSString::from_str(path_str);
    let url = NSURL::fileURLWithPath_isDirectory(&ns_path, true);

    let data = url
        .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
            NSURLBookmarkCreationOptions::WithSecurityScope,
            None,
            None,
        )
        .map_err(|e| SecurityScopedError::Foundation(describe_ns_error(&e)))?;

    Ok(data.to_vec())
}

pub fn resolve_bookmark(blob: &[u8]) -> Result<ResolvedBookmark, SecurityScopedError> {
    let (url, is_stale) = resolve_bookmark_url(blob)?;
    let path = path_buf_from_file_url(&url)?;
    Ok(ResolvedBookmark { path, is_stale })
}

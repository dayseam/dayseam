//! macOS security-scoped bookmarks and RAII access (**MAS-4b**).
//!
//! Implements [`docs/design/2026-phase-5-mas-architecture.md`](../../../../../docs/design/2026-phase-5-mas-architecture.md)
//! §9.5 (RAII) and the bookmark bytes surface described in §9.6. Non-macOS targets
//! compile with stubs so Linux CI keeps building the desktop crate.

use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod stub;

#[cfg(target_os = "macos")]
use macos as imp;
#[cfg(not(target_os = "macos"))]
use stub as imp;

/// Outcome of resolving bookmark bytes to a filesystem location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBookmark {
    pub path: PathBuf,
    /// Set when Foundation reports the bookmark no longer matches the on-disk item.
    pub is_stale: bool,
}

/// Errors from bookmark creation, resolution, or security-scoped access.
#[derive(Debug)]
pub enum SecurityScopedError {
    /// Non-macOS builds do not support AppKit / Foundation bookmark APIs.
    UnsupportedPlatform,
    /// Bookmark byte slice was empty.
    EmptyBookmarkData,
    /// Path is not valid UTF-8 (required for `NSString` bridging).
    NonUtf8Path,
    /// Expected a directory (scan root / sink folder).
    NotADirectory,
    /// Path does not exist or could not be read.
    Io(std::io::Error),
    /// `NSURL` bookmark or resolve failed (`NSError` from Foundation).
    Foundation(String),
    /// `startAccessingSecurityScopedResource` returned false.
    StartAccessDenied,
}

impl fmt::Display for SecurityScopedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SecurityScopedError::UnsupportedPlatform => {
                write!(f, "security-scoped bookmarks are only supported on macOS")
            }
            SecurityScopedError::EmptyBookmarkData => write!(f, "bookmark data is empty"),
            SecurityScopedError::NonUtf8Path => write!(f, "path is not valid UTF-8"),
            SecurityScopedError::NotADirectory => write!(f, "path is not a directory"),
            SecurityScopedError::Io(e) => write!(f, "{e}"),
            SecurityScopedError::Foundation(msg) => write!(f, "Foundation error: {msg}"),
            SecurityScopedError::StartAccessDenied => {
                write!(f, "startAccessingSecurityScopedResource returned false")
            }
        }
    }
}

impl std::error::Error for SecurityScopedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SecurityScopedError::Io(e) => Some(e),
            _ => None,
        }
    }
}

/// Create security-scoped bookmark bytes for an existing directory.
///
/// The path is canonicalized before encoding ([`docs/design/2026-phase-5-mas-architecture.md`](../../../../../docs/design/2026-phase-5-mas-architecture.md) §9.4).
pub fn create_directory_bookmark(path: &Path) -> Result<Vec<u8>, SecurityScopedError> {
    imp::create_directory_bookmark(path)
}

/// Resolve bookmark bytes to a filesystem path and stale flag.
pub fn resolve_bookmark(blob: &[u8]) -> Result<ResolvedBookmark, SecurityScopedError> {
    imp::resolve_bookmark(blob)
}

/// RAII guard: [`Drop`] calls `stopAccessingSecurityScopedResource` for every successful
/// [`SecurityScopedGuard::new`](Self::new) / [`SecurityScopedGuard::from_bookmark`](Self::from_bookmark).
pub struct SecurityScopedGuard {
    inner: imp::SecurityScopedGuardInner,
}

impl SecurityScopedGuard {
    /// Begin security-scoped access using a filesystem path (non-sandbox / tests).
    ///
    /// For sandboxed App Store builds, use [`Self::from_bookmark`] so access pairs with the
    /// resolved bookmark URL.
    pub fn new(path: &Path) -> Result<Self, SecurityScopedError> {
        Ok(Self {
            inner: imp::SecurityScopedGuardInner::new(path)?,
        })
    }

    /// Resolve security-scoped bookmark bytes and begin access on the resulting `NSURL`.
    ///
    /// This is the preferred entry point after persisting [`create_directory_bookmark`] output.
    pub fn from_bookmark(blob: &[u8]) -> Result<Self, SecurityScopedError> {
        Ok(Self {
            inner: imp::SecurityScopedGuardInner::from_bookmark(blob)?,
        })
    }
}

impl Drop for SecurityScopedGuard {
    fn drop(&mut self) {
        self.inner.stop();
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn round_trip_bookmark_and_guard_writes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_path = dir.path().to_path_buf();
        let dir_canon = fs::canonicalize(&dir_path).unwrap();

        let blob = create_directory_bookmark(&dir_path).expect("create bookmark");
        assert!(!blob.is_empty());

        let resolved = resolve_bookmark(&blob).expect("resolve bookmark");
        assert_eq!(
            fs::canonicalize(&resolved.path).expect("canonicalize resolved"),
            dir_canon
        );
        assert!(!resolved.is_stale);

        let child = dir_path.join("mas4b.txt");
        {
            let _guard = SecurityScopedGuard::from_bookmark(&blob).expect("start access");
            fs::write(&child, b"ok").expect("write under guard");
        }

        assert_eq!(fs::read_to_string(&child).unwrap(), "ok");
    }

    #[test]
    fn reject_non_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, b"x").unwrap();
        assert!(matches!(
            create_directory_bookmark(&file_path),
            Err(SecurityScopedError::NotADirectory)
        ));
    }

    #[test]
    fn reject_empty_bookmark_bytes() {
        assert!(matches!(
            resolve_bookmark(&[]),
            Err(SecurityScopedError::EmptyBookmarkData)
        ));
        assert!(matches!(
            SecurityScopedGuard::from_bookmark(&[]),
            Err(SecurityScopedError::EmptyBookmarkData)
        ));
    }
}

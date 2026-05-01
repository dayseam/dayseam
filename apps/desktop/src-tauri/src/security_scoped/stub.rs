//! Non-macOS stubs: keep `dayseam-desktop` building on Linux CI without Foundation.

use std::path::Path;

use super::{ResolvedBookmark, SecurityScopedError};

pub struct SecurityScopedGuardInner;

impl SecurityScopedGuardInner {
    pub fn new(_path: &Path) -> Result<Self, SecurityScopedError> {
        Err(SecurityScopedError::UnsupportedPlatform)
    }

    pub fn from_bookmark(_blob: &[u8]) -> Result<Self, SecurityScopedError> {
        Err(SecurityScopedError::UnsupportedPlatform)
    }

    pub fn stop(&mut self) {}
}

pub fn create_directory_bookmark(_path: &Path) -> Result<Vec<u8>, SecurityScopedError> {
    Err(SecurityScopedError::UnsupportedPlatform)
}

pub fn resolve_bookmark(_blob: &[u8]) -> Result<ResolvedBookmark, SecurityScopedError> {
    Err(SecurityScopedError::UnsupportedPlatform)
}

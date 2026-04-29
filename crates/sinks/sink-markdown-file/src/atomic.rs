//! Write-to-temp-then-rename helper + orphan-sweep for interrupted
//! writes.
//!
//! Every `write()` produces a sibling file
//! `{dest_dir}/.{filename}.{nanos}.dayseam.tmp`, flushes + fsyncs it,
//! then calls `rename(2)` / `MoveFileExReplaceExisting` to atomically
//! swing the target path onto the new inode. The rename is the single
//! observable mutation of the target; a crash before `rename` leaves
//! the original file untouched and at worst drops a
//! `.dayseam.tmp`-suffixed sibling in the directory.
//!
//! [`sweep_orphans`] is called at sink-init time and deletes any
//! `.dayseam.tmp`-suffixed sibling older than [`STALE_TMP_AGE`] so a
//! crashed run does not leave the user's destination folder slowly
//! accumulating debris.
//!
//! ## Why hand-rolled, not `tempfile::NamedTempFile`
//!
//! `NamedTempFile::persist` is atomic and well-tested, but its default
//! random naming (`.tmpXXXX`) is indistinguishable from a Vim swap or
//! editor backup — our orphan sweep would either be too greedy
//! (clobbering unrelated files) or too paranoid (leaving our own
//! debris in place). Namespacing the suffix explicitly with
//! `.dayseam.tmp` means the sweep pattern is unambiguous.
//!
//! ## DAY-187 hardening (audit follow-up)
//!
//! - `rename(2)` over a symlink at the final component would silently
//!   cause the kernel to overwrite whatever the symlink points to.
//!   Combined with the read-side gap that
//!   [`crate::adapter::read_target_if_any`] now plugs, this turned the
//!   sink into a write-anywhere primitive on shared POSIX hosts. The
//!   fix is to `symlink_metadata` the target before rename and refuse
//!   if it is a symlink — exactly mirroring the read-side guard.
//! - On POSIX, the rename promotes the *temp file's* inode (with its
//!   own permission bits) into the target slot and unlinks the
//!   previous target. A naive temp-file open inherits umask, which on
//!   most Linux distros is `0022` and produces `0644` — meaning every
//!   save promoted a freshly-readable-by-others file in place of a
//!   user-tightened `0600` daily note. The fix is to capture the
//!   pre-existing target's mode and reapply it to the temp file
//!   before rename; new files default to `0600` so a brand-new daily
//!   note is private by default.

use std::fs;
use std::io::{self, Write as _};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use dayseam_core::{error_codes, DayseamError};
use tracing::{debug, warn};

/// Files matching [`TMP_SUFFIX`] or [`LOCK_SUFFIX`] older than this
/// threshold are removed on sink init. Five minutes is long enough to
/// cover a slow IPC write and short enough that a crash-recovered
/// user does not see old cruft on their next Generate-Save cycle.
pub(crate) const STALE_TMP_AGE: Duration = Duration::from_secs(5 * 60);

/// Suffix every Dayseam-authored temp file wears. Kept public to the
/// module so the writer and the sweep share one literal.
pub(crate) const TMP_SUFFIX: &str = ".dayseam.tmp";

/// Suffix of the lock sentinel from `crate::lock`, duplicated here
/// (rather than imported) so the sweep is self-contained and the
/// `lock` module can remain a leaf that the adapter composes on top.
const LOCK_SUFFIX: &str = ".dayseam.lock";

/// POSIX permission bits applied to a brand-new daily-note file
/// (DAY-187 audit M1). The previous default was umask-derived (most
/// commonly `0644`), which left freshly-created daily notes
/// world-readable on shared multi-tenant hosts. `0600` matches the
/// implicit "this is my journal" expectation users have when they
/// type into Dayseam and is consistent with how
/// `crates/dayseam-secrets` already provisions its own state files.
#[cfg(unix)]
const NEW_FILE_MODE: u32 = 0o600;

/// Atomically write `bytes` to `target`: sibling temp file in the
/// same directory, fsync, rename. On success the return value is
/// `bytes.len()` so callers can feed it directly into
/// `WriteReceipt::bytes_written`.
///
/// On POSIX, the temp file's permission bits are aligned to either
/// (a) the existing target's mode if it exists, or (b) `0600` if
/// the target is brand-new — see the module docstring for the
/// "promoted-temp-inode" rationale.
///
/// Refuses to write when `target` is a symlink (DAY-187 H3). The
/// caller is expected to surface the resulting [`DayseamError::Io`]
/// with `code = sink.fs.refused_symlink` to the user.
pub(crate) fn atomic_write(target: &Path, bytes: &[u8]) -> Result<u64, DayseamError> {
    // Stat the existing target (if any). Used for two checks:
    // 1. Symlink refusal (mirrors the read-side guard in adapter.rs).
    // 2. POSIX mode preservation when the target already exists.
    let existing_meta = match fs::symlink_metadata(target) {
        Ok(m) => Some(m),
        Err(err) if err.kind() == io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(DayseamError::Io {
                code: error_codes::SINK_FS_NOT_WRITABLE.to_string(),
                path: Some(target.to_path_buf()),
                message: format!("could not stat target before rename: {err}"),
            });
        }
    };

    if let Some(meta) = &existing_meta {
        if meta.file_type().is_symlink() {
            return Err(DayseamError::Io {
                code: error_codes::SINK_FS_REFUSED_SYMLINK.to_string(),
                path: Some(target.to_path_buf()),
                message: format!(
                    "{} is a symlink; refusing to rename over it (see sink.fs.refused_symlink)",
                    target.display()
                ),
            });
        }
    }

    atomic_write_impl(target, bytes, existing_meta.as_ref()).map_err(|err| DayseamError::Io {
        code: error_codes::SINK_FS_NOT_WRITABLE.to_string(),
        path: Some(target.to_path_buf()),
        message: format!("atomic write failed: {err}"),
    })
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn atomic_write_impl(
    target: &Path,
    bytes: &[u8],
    existing_meta: Option<&fs::Metadata>,
) -> io::Result<u64> {
    let (parent, filename) = split_target(target)?;
    let tmp_path = sibling_tmp_path(parent, filename);

    // `create_new(true)` guards against the astronomically unlikely
    // case where `sibling_tmp_path` collided with an existing file.
    // If it does, the next call (one nanosecond later) will pick a
    // fresh name.
    let (mut file, tmp_path) = match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp_path)
    {
        Ok(f) => (f, tmp_path),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            // Retry exactly once with a freshly-nano-suffixed name.
            let retry = sibling_tmp_path(parent, filename);
            let f = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&retry)?;
            (f, retry)
        }
        Err(e) => return Err(e),
    };

    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);

    #[cfg(unix)]
    {
        // DAY-187 (Data H7): align the temp file's mode to the
        // target's pre-existing mode (if any) before the rename so
        // the user's `chmod` choice survives every save. New files
        // default to `0600` per the module docstring.
        let mode = existing_meta
            .map(|m| m.permissions().mode() & 0o7777)
            .unwrap_or(NEW_FILE_MODE);
        // Best-effort: a Permissions failure here would leave the
        // temp file at its default mode. Surface as an Io error so
        // the caller can decide; on most platforms this only fails
        // if the FS does not support permissions (FAT/NTFS via WSL
        // and similar), in which case the rename below would still
        // succeed but the mode would be uncontrolled.
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(mode))?;
    }

    // `fs::rename` is atomic on POSIX and uses `MoveFileExW` with
    // `MOVEFILE_REPLACE_EXISTING` on Windows.
    fs::rename(&tmp_path, target)?;
    Ok(bytes.len() as u64)
}

/// Remove any `*.dayseam.tmp` files under `dir` older than
/// [`STALE_TMP_AGE`]. Errors are logged and otherwise swallowed; this
/// is a best-effort cleanup, not a correctness requirement.
pub(crate) fn sweep_orphans(dir: &Path) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            debug!(
                target = "sink-markdown-file",
                dir = %dir.display(),
                error = %err,
                "orphan sweep: could not read destination directory"
            );
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !is_sweep_target(&path) {
            continue;
        }
        let age = match entry.metadata().and_then(|m| m.modified()) {
            Ok(mtime) => SystemTime::now()
                .duration_since(mtime)
                .unwrap_or(Duration::ZERO),
            Err(err) => {
                debug!(
                    target = "sink-markdown-file",
                    path = %path.display(),
                    error = %err,
                    "orphan sweep: could not stat file, skipping"
                );
                continue;
            }
        };
        if age < STALE_TMP_AGE {
            continue;
        }
        match fs::remove_file(&path) {
            Ok(_) => debug!(
                target = "sink-markdown-file",
                path = %path.display(),
                age_secs = age.as_secs(),
                "orphan sweep: removed stale temp file"
            ),
            Err(err) => warn!(
                target = "sink-markdown-file",
                path = %path.display(),
                error = %err,
                "orphan sweep: could not remove stale temp file"
            ),
        }
    }
}

fn split_target(target: &Path) -> io::Result<(&Path, &str)> {
    let parent = target.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("target has no parent directory: {}", target.display()),
        )
    })?;
    let filename = target.file_name().and_then(|s| s.to_str()).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("target has no filename: {}", target.display()),
        )
    })?;
    Ok((parent, filename))
}

fn sibling_tmp_path(parent: &Path, filename: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    // Leading dot keeps the temp out of Finder / most file listings;
    // the explicit suffix keeps the sweep pattern unambiguous.
    parent.join(format!(".{filename}.{nanos:08x}{TMP_SUFFIX}"))
}

/// Returns `true` if `path`'s filename ends with [`TMP_SUFFIX`]. Used
/// by the unit tests below.
#[cfg(test)]
fn is_dayseam_tmp(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.ends_with(TMP_SUFFIX))
}

/// Match either a stalled temp file or a stalled lock sentinel. Both
/// are best-effort cleanup targets: a live writer creates a lock and
/// deletes it on drop, so any lock file older than `STALE_TMP_AGE` is
/// necessarily the artefact of a crash.
fn is_sweep_target(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .is_some_and(|s| s.ends_with(TMP_SUFFIX) || s.ends_with(LOCK_SUFFIX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_target_with_contents() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.md");
        let bytes = b"hello world\n";
        let n = atomic_write(&target, bytes).unwrap();
        assert_eq!(n, bytes.len() as u64);
        assert_eq!(fs::read(&target).unwrap(), bytes);
    }

    #[test]
    fn atomic_write_rewrites_target_without_leaving_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("out.md");
        atomic_write(&target, b"first\n").unwrap();
        atomic_write(&target, b"second\n").unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"second\n");
        let stragglers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| is_dayseam_tmp(&e.path()))
            .collect();
        assert!(
            stragglers.is_empty(),
            "no `.dayseam.tmp` files should remain after a successful atomic_write; found {stragglers:?}"
        );
    }

    #[test]
    fn sweep_orphans_removes_stale_tmp_files() {
        let dir = tempfile::tempdir().unwrap();
        let stale = dir
            .path()
            .join(".Dayseam 2026-04-18.md.abcdef12.dayseam.tmp");
        fs::write(&stale, b"corpse").unwrap();
        set_modified(&stale, SystemTime::now() - (STALE_TMP_AGE * 2));

        let fresh = dir
            .path()
            .join(".Dayseam 2026-04-19.md.99999999.dayseam.tmp");
        fs::write(&fresh, b"in-flight").unwrap();

        sweep_orphans(dir.path());

        assert!(!stale.exists(), "stale orphan must be swept");
        assert!(fresh.exists(), "fresh orphan must survive");
    }

    #[test]
    fn sweep_orphans_leaves_unrelated_files_alone() {
        let dir = tempfile::tempdir().unwrap();
        let user_md = dir.path().join("Dayseam 2026-04-18.md");
        fs::write(&user_md, b"user content").unwrap();
        set_modified(&user_md, SystemTime::now() - (STALE_TMP_AGE * 3));

        let unrelated_tmp = dir.path().join("something.tmp");
        fs::write(&unrelated_tmp, b"not ours").unwrap();
        set_modified(&unrelated_tmp, SystemTime::now() - (STALE_TMP_AGE * 3));

        sweep_orphans(dir.path());

        assert!(user_md.exists(), "sweep must not touch non-tmp files");
        assert!(
            unrelated_tmp.exists(),
            "sweep must not touch files outside our namespaced suffix"
        );
    }

    #[test]
    fn sweep_orphans_removes_stale_lock_sentinels() {
        let dir = tempfile::tempdir().unwrap();
        let stale_lock = dir.path().join("Dayseam 2026-04-18.md.dayseam.lock");
        fs::write(&stale_lock, b"").unwrap();
        set_modified(&stale_lock, SystemTime::now() - (STALE_TMP_AGE * 2));
        sweep_orphans(dir.path());
        assert!(!stale_lock.exists(), "stale lock sentinel must be swept");
    }

    #[test]
    fn atomic_write_refuses_when_parent_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("does-not-exist").join("out.md");
        let err = atomic_write(&target, b"content").unwrap_err();
        // `create_new` on a path whose parent is missing yields
        // NotFound, surfaced to the caller as `sink.fs.not_writable`
        // (the umbrella code we already use for any I/O failure
        // during the temp-file write phase).
        assert_eq!(err.code(), error_codes::SINK_FS_NOT_WRITABLE);
    }

    /// DAY-187 H3 regression: the sink must NOT silently follow a
    /// symlink at the final component on rename. A local attacker
    /// that landed `Dayseam <date>.md → /etc/passwd` in the
    /// destination directory would otherwise turn the next save into
    /// an arbitrary-write primitive (the rename swings the symlink
    /// target onto whatever bytes the orchestrator just rendered).
    #[cfg(unix)]
    #[test]
    fn atomic_write_refuses_to_follow_symlink_target() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Dayseam 2026-04-18.md");
        let pointed_at = dir.path().join("victim.md");
        fs::write(&pointed_at, b"original victim content\n").unwrap();
        symlink(&pointed_at, &target).unwrap();

        let err = atomic_write(&target, b"attacker payload\n").unwrap_err();
        assert_eq!(err.code(), error_codes::SINK_FS_REFUSED_SYMLINK);
        // The pointed-at file must be untouched after the refusal.
        assert_eq!(fs::read(&pointed_at).unwrap(), b"original victim content\n");
    }

    /// DAY-187 H7: a user who `chmod 0640`s their daily note expects
    /// that mode to survive subsequent saves. The previous shape
    /// inherited umask on every save and silently slid back to
    /// `0644` (or whatever umask happens to be on that machine).
    #[cfg(unix)]
    #[test]
    fn atomic_write_preserves_existing_target_mode() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Dayseam 2026-04-18.md");
        fs::write(&target, b"v1\n").unwrap();
        // Pick an unusual non-default mode so the test fails loudly
        // if the implementation forgets to copy it.
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).unwrap();

        atomic_write(&target, b"v2\n").unwrap();

        let mode_after = fs::metadata(&target).unwrap().permissions().mode() & 0o7777;
        assert_eq!(
            mode_after, 0o640,
            "save must preserve the user's chosen mode, got 0o{mode_after:o}"
        );
    }

    /// DAY-187 M1: a brand-new daily-note file is created with mode
    /// `0600` (private to the user) rather than the umask-derived
    /// default. This matches the implicit "this is my journal"
    /// expectation users have when they type into Dayseam.
    #[cfg(unix)]
    #[test]
    fn atomic_write_creates_new_files_as_0600() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Dayseam 2026-04-18.md");
        atomic_write(&target, b"hello\n").unwrap();

        let mode = fs::metadata(&target).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o600, "new files should be 0o600, got 0o{mode:o}");
    }

    fn set_modified(path: &Path, when: SystemTime) {
        let f = fs::OpenOptions::new().write(true).open(path).unwrap();
        f.set_modified(when).unwrap();
    }
}

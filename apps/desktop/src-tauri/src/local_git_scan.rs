//! Local Git discovery with optional MAS security-scoped access (**MAS-4c**, **MAS-4f**).
//!
//! When the `mas` feature is enabled on macOS, [`discover_scan_roots_with_optional_mas_access`]
//! resolves persisted bookmark blobs from [`dayseam_db::SecurityScopedBookmarkRepo`] and wraps
//! each scan root’s walk in [`crate::security_scoped::SecurityScopedGuard`] when a blob exists.
//! Stale or unusable bookmarks are detected via [`crate::security_scoped::resolve_bookmark`];
//! discovery falls back to unscoped walks and reports affected roots in
//! [`LocalGitDiscoveryResult::stale_bookmarked_scan_roots`] (also preserved on
//! [`LocalGitDiscoveryError`] when a later root aborts the walk) so the IPC layer can surface UX.
//! Other builds behave like the direct SKU (single `discover_repos` call).
//!
//! **`max_roots` parity:** [`connector_local_git::discover_repos`] applies one global cap across
//! all scan roots and stops scanning further roots once the walker truncates. Per-root MAS walks
//! use the same cumulative budget (`remaining = max_roots − merged.len()`) and stop when any inner
//! walk sets `truncated`, matching that behaviour.

use std::path::PathBuf;

use connector_local_git::{discover_repos, DiscoveryConfig, DiscoveryOutcome};
use dayseam_core::{DayseamError, SourceId};
use sqlx::SqlitePool;

/// Outcome of [`discover_scan_roots_with_optional_mas_access`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalGitDiscoveryResult {
    pub outcome: DiscoveryOutcome,
    /// Scan roots where a persisted bookmark could not be resolved, was marked stale by
    /// Foundation, or could not start security-scoped access — discovery may have continued
    /// without sandbox grants. Callers should nudge the user to re-select those folders.
    pub stale_bookmarked_scan_roots: Vec<PathBuf>,
}

/// Failure from [`discover_scan_roots_with_optional_mas_access`].
///
/// Carries [`LocalGitDiscoveryResult::stale_bookmarked_scan_roots`] collected **before** the
/// walk aborted (e.g. missing scan root) so callers can still surface the stale-folder toast.
#[derive(Debug)]
pub(crate) struct LocalGitDiscoveryError {
    pub source: DayseamError,
    pub stale_bookmarked_scan_roots: Vec<PathBuf>,
}

/// Discover repos under `scan_roots`, using per-root security scope on **macOS + `mas`**
/// when `security_scoped_bookmarks.bookmark_blob` is populated for that root.
pub(crate) async fn discover_scan_roots_with_optional_mas_access(
    pool: SqlitePool,
    source_id: &SourceId,
    scan_roots: &[PathBuf],
) -> Result<LocalGitDiscoveryResult, LocalGitDiscoveryError> {
    if scan_roots.is_empty() {
        return Ok(LocalGitDiscoveryResult {
            outcome: DiscoveryOutcome {
                repos: vec![],
                truncated: false,
            },
            stale_bookmarked_scan_roots: vec![],
        });
    }

    #[cfg(not(feature = "mas"))]
    {
        let _ = (pool, source_id);
        let outcome = discover_repos(scan_roots, DiscoveryConfig::default()).map_err(|e| {
            LocalGitDiscoveryError {
                source: e,
                stale_bookmarked_scan_roots: vec![],
            }
        })?;
        Ok(LocalGitDiscoveryResult {
            outcome,
            stale_bookmarked_scan_roots: vec![],
        })
    }

    #[cfg(feature = "mas")]
    {
        #[cfg(target_os = "macos")]
        {
            mas_macos_discover(pool, source_id, scan_roots).await
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (pool, source_id);
            let outcome = discover_repos(scan_roots, DiscoveryConfig::default()).map_err(|e| {
                LocalGitDiscoveryError {
                    source: e,
                    stale_bookmarked_scan_roots: vec![],
                }
            })?;
            Ok(LocalGitDiscoveryResult {
                outcome,
                stale_bookmarked_scan_roots: vec![],
            })
        }
    }
}

#[cfg(all(feature = "mas", target_os = "macos"))]
fn cmp_paths(a: &std::path::Path, b: &std::path::Path) -> std::cmp::Ordering {
    a.as_os_str().cmp(b.as_os_str())
}

#[cfg(all(feature = "mas", target_os = "macos"))]
async fn mas_macos_discover(
    pool: SqlitePool,
    source_id: &SourceId,
    scan_roots: &[PathBuf],
) -> Result<LocalGitDiscoveryResult, LocalGitDiscoveryError> {
    use dayseam_db::SecurityScopedBookmarkRepo;

    use crate::security_scoped::{resolve_bookmark, SecurityScopedGuard};

    let repo = SecurityScopedBookmarkRepo::new(pool);
    let rows = repo
        .list_local_git_scan_rows(source_id)
        .await
        .map_err(|e| LocalGitDiscoveryError {
            source: DayseamError::Internal {
                code: "local_git_scan.bookmarks_list".into(),
                message: e.to_string(),
            },
            stale_bookmarked_scan_roots: vec![],
        })?;

    let cfg = DiscoveryConfig::default();
    let mut merged = Vec::new();
    let mut truncated = false;
    let mut stale_bookmarked_scan_roots: Vec<PathBuf> = Vec::new();

    let note_stale_root = |root: &std::path::Path, bucket: &mut Vec<PathBuf>| {
        let p = root.to_path_buf();
        if !bucket.contains(&p) {
            bucket.push(p);
        }
    };

    for root in scan_roots {
        // Mirror `discover_repos`: one shared `max_roots` budget across all scan roots, and no
        // further roots after the walker truncates (`discovery.rs` breaks the outer `scan_roots`
        // loop when `truncated`).
        if truncated {
            break;
        }

        let remaining = cfg.max_roots.saturating_sub(merged.len());
        if remaining == 0 {
            truncated = true;
            break;
        }

        let sub_cfg = DiscoveryConfig {
            max_depth: cfg.max_depth,
            max_roots: remaining,
        };

        let key = dayseam_db::local_git_scan_root_logical_path(root).map_err(|e| {
            LocalGitDiscoveryError {
                source: DayseamError::Internal {
                    code: "local_git_scan.logical_path".into(),
                    message: e.to_string(),
                },
                stale_bookmarked_scan_roots: std::mem::take(&mut stale_bookmarked_scan_roots),
            }
        })?;

        let blob = rows
            .iter()
            .find(|r| r.logical_path == key)
            .and_then(|r| r.bookmark_blob.as_ref());

        let per_root = if let Some(bytes) = blob {
            match resolve_bookmark(bytes) {
                Err(e) => {
                    note_stale_root(root, &mut stale_bookmarked_scan_roots);
                    tracing::warn!(
                        source_id = %source_id,
                        root = %root.display(),
                        error = %e,
                        "MAS: could not resolve saved security-scoped bookmark; falling back to unscoped discovery"
                    );
                    discover_repos(std::slice::from_ref(root), sub_cfg)
                }
                Ok(resolved) => {
                    if resolved.is_stale {
                        note_stale_root(root, &mut stale_bookmarked_scan_roots);
                        tracing::warn!(
                            source_id = %source_id,
                            root = %root.display(),
                            resolved_path = %resolved.path.display(),
                            "MAS: bookmark marked stale by Foundation; user should re-select this scan root in Settings"
                        );
                    }
                    match SecurityScopedGuard::from_bookmark(bytes) {
                        Ok(_guard) => discover_repos(std::slice::from_ref(root), sub_cfg),
                        Err(e) => {
                            if !resolved.is_stale {
                                note_stale_root(root, &mut stale_bookmarked_scan_roots);
                            }
                            tracing::warn!(
                                source_id = %source_id,
                                root = %root.display(),
                                error = %e,
                                "MAS: bookmark resolved but security-scoped access failed; trying unscoped discovery"
                            );
                            discover_repos(std::slice::from_ref(root), sub_cfg)
                        }
                    }
                }
            }
        } else {
            tracing::trace!(
                source_id = %source_id,
                root = %root.display(),
                "MAS: no bookmark blob yet; unscoped discovery"
            );
            discover_repos(std::slice::from_ref(root), sub_cfg)
        };

        match per_root {
            Ok(o) => {
                merged.extend(o.repos);
                truncated |= o.truncated;
                if truncated {
                    break;
                }
            }
            Err(e) => {
                return Err(LocalGitDiscoveryError {
                    source: e,
                    stale_bookmarked_scan_roots,
                });
            }
        }
    }

    merged.sort_by(|a, b| cmp_paths(&a.path, &b.path));
    merged.dedup_by(|a, b| a.path == b.path);

    stale_bookmarked_scan_roots.sort_by(|a, b| cmp_paths(a.as_path(), b.as_path()));
    stale_bookmarked_scan_roots.dedup();

    Ok(LocalGitDiscoveryResult {
        outcome: DiscoveryOutcome {
            repos: merged,
            truncated,
        },
        stale_bookmarked_scan_roots,
    })
}

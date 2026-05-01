//! Local Git discovery with optional MAS security-scoped access (**MAS-4c**).
//!
//! When the `mas` feature is enabled on macOS, [`discover_scan_roots_with_optional_mas_access`]
//! resolves persisted bookmark blobs from [`dayseam_db::SecurityScopedBookmarkRepo`] and wraps
//! each scan root’s walk in [`crate::security_scoped::SecurityScopedGuard`] when a blob exists.
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

/// Discover repos under `scan_roots`, using per-root security scope on **macOS + `mas`**
/// when `security_scoped_bookmarks.bookmark_blob` is populated for that root.
pub async fn discover_scan_roots_with_optional_mas_access(
    pool: SqlitePool,
    source_id: &SourceId,
    scan_roots: &[PathBuf],
) -> Result<DiscoveryOutcome, DayseamError> {
    if scan_roots.is_empty() {
        return Ok(DiscoveryOutcome {
            repos: vec![],
            truncated: false,
        });
    }

    #[cfg(not(feature = "mas"))]
    {
        let _ = (pool, source_id);
        discover_repos(scan_roots, DiscoveryConfig::default())
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
            discover_repos(scan_roots, DiscoveryConfig::default())
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
) -> Result<DiscoveryOutcome, DayseamError> {
    use dayseam_db::SecurityScopedBookmarkRepo;

    use crate::security_scoped::SecurityScopedGuard;

    let repo = SecurityScopedBookmarkRepo::new(pool);
    let rows = repo
        .list_local_git_scan_rows(source_id)
        .await
        .map_err(|e| DayseamError::Internal {
            code: "local_git_scan.bookmarks_list".into(),
            message: e.to_string(),
        })?;

    let cfg = DiscoveryConfig::default();
    let mut merged = Vec::new();
    let mut truncated = false;

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
            DayseamError::Internal {
                code: "local_git_scan.logical_path".into(),
                message: e.to_string(),
            }
        })?;

        let blob = rows
            .iter()
            .find(|r| r.logical_path == key)
            .and_then(|r| r.bookmark_blob.as_ref());

        let per_root = if let Some(bytes) = blob {
            match SecurityScopedGuard::from_bookmark(bytes) {
                Ok(_guard) => discover_repos(std::slice::from_ref(root), sub_cfg),
                Err(e) => {
                    tracing::debug!(
                        source_id = %source_id,
                        root = %root.display(),
                        error = %e,
                        "MAS: bookmark guard failed; trying unscoped discovery"
                    );
                    discover_repos(std::slice::from_ref(root), sub_cfg)
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
            Err(e) => return Err(e),
        }
    }

    merged.sort_by(|a, b| cmp_paths(&a.path, &b.path));
    merged.dedup_by(|a, b| a.path == b.path);

    Ok(DiscoveryOutcome {
        repos: merged,
        truncated,
    })
}

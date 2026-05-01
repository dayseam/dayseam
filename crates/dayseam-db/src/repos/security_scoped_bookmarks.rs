//! `security_scoped_bookmarks` — MAS-4a table; **MAS-4c** sync for Local Git scan roots,
//! **MAS-4d** sync for Markdown-file sink `dest_dirs`.

use std::collections::HashSet;
use std::path::Path;

use dayseam_core::SourceId;
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{DbError, DbResult};

/// `logical_path` value that matches `sources.config_json` (`LocalGit.scan_roots[]`).
///
/// Uses the same UTF-8 string form as `local_repos.path` and other DB path columns
/// (`Path::to_str`), which matches `serde_json` encoding of `PathBuf` for typical paths.
pub fn local_git_scan_root_logical_path(path: &Path) -> DbResult<String> {
    path.to_str()
        .map(String::from)
        .ok_or_else(|| DbError::InvalidData {
            column: "security_scoped_bookmarks.logical_path".into(),
            message: format!("scan root path is not valid UTF-8: {path:?}"),
        })
}

/// `logical_path` value that matches `sinks.config_json` (`MarkdownFile.dest_dirs[]`).
///
/// Uses the same UTF-8 string form as [`local_git_scan_root_logical_path`].
pub fn markdown_sink_dest_logical_path(path: &Path) -> DbResult<String> {
    path.to_str()
        .map(String::from)
        .ok_or_else(|| DbError::InvalidData {
            column: "security_scoped_bookmarks.logical_path".into(),
            message: format!("markdown sink dest path is not valid UTF-8: {path:?}"),
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalGitScanBookmarkRow {
    pub logical_path: String,
    pub bookmark_blob: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownSinkDestBookmarkRow {
    pub logical_path: String,
    pub bookmark_blob: Option<Vec<u8>>,
}

#[derive(Clone)]
pub struct SecurityScopedBookmarkRepo {
    pool: SqlitePool,
}

impl SecurityScopedBookmarkRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_local_git_scan_rows(
        &self,
        source_id: &SourceId,
    ) -> DbResult<Vec<LocalGitScanBookmarkRow>> {
        let rows = sqlx::query(
            "SELECT logical_path, bookmark_blob
             FROM security_scoped_bookmarks
             WHERE owner_source_id = ? AND role = 'local_git_scan_root'
             ORDER BY logical_path ASC",
        )
        .bind(source_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_scan_row).collect()
    }

    /// Keep `security_scoped_bookmarks` rows aligned with `LocalGit.scan_roots`.
    ///
    /// - Removes rows whose `logical_path` is no longer in `scan_roots`.
    /// - Inserts placeholder rows (`bookmark_blob` NULL) for new paths without touching
    ///   existing blobs for paths that remain (**MAS-4e** fills blobs after `dialog.open`).
    pub async fn sync_local_git_scan_roots(
        &self,
        source_id: &SourceId,
        scan_roots: &[std::path::PathBuf],
    ) -> DbResult<()> {
        let sid = source_id.to_string();

        if scan_roots.is_empty() {
            sqlx::query(
                "DELETE FROM security_scoped_bookmarks
                 WHERE owner_source_id = ? AND role = 'local_git_scan_root'",
            )
            .bind(&sid)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.delete_all_roots"))?;
            return Ok(());
        }

        let mut keys = Vec::with_capacity(scan_roots.len());
        for p in scan_roots {
            keys.push(local_git_scan_root_logical_path(p)?);
        }
        let key_set: HashSet<&str> = keys.iter().map(String::as_str).collect();

        let existing = self.list_local_git_scan_rows(source_id).await?;
        for row in existing {
            if !key_set.contains(row.logical_path.as_str()) {
                sqlx::query(
                    "DELETE FROM security_scoped_bookmarks
                     WHERE owner_source_id = ? AND role = 'local_git_scan_root'
                       AND logical_path = ?",
                )
                .bind(&sid)
                .bind(&row.logical_path)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.delete_stale"))?;
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        for path in scan_roots {
            let logical = local_git_scan_root_logical_path(path)?;
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM security_scoped_bookmarks
                 WHERE owner_source_id = ? AND role = 'local_git_scan_root'
                   AND logical_path = ?",
            )
            .bind(&sid)
            .bind(&logical)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.count"))?;

            if count == 0 {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"INSERT INTO security_scoped_bookmarks
                        (id, owner_source_id, owner_sink_id, role, logical_path,
                         bookmark_blob, meta_json, created_at, updated_at)
                       VALUES (?, ?, NULL, 'local_git_scan_root', ?, NULL, NULL, ?, ?)"#,
                )
                .bind(&id)
                .bind(&sid)
                .bind(&logical)
                .bind(&now)
                .bind(&now)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.insert_root"))?;
            }
        }

        Ok(())
    }

    pub async fn list_markdown_sink_dest_rows(
        &self,
        sink_id: &Uuid,
    ) -> DbResult<Vec<MarkdownSinkDestBookmarkRow>> {
        let rows = sqlx::query(
            "SELECT logical_path, bookmark_blob
             FROM security_scoped_bookmarks
             WHERE owner_sink_id = ? AND role = 'markdown_sink_dest'
             ORDER BY logical_path ASC",
        )
        .bind(sink_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(row_to_sink_dest_row).collect()
    }

    /// Keep `security_scoped_bookmarks` rows aligned with `MarkdownFile.dest_dirs`.
    ///
    /// Same placeholder / blob-preservation rules as [`Self::sync_local_git_scan_roots`].
    pub async fn sync_markdown_sink_dest_dirs(
        &self,
        sink_id: &Uuid,
        dest_dirs: &[std::path::PathBuf],
    ) -> DbResult<()> {
        let sid = sink_id.to_string();

        if dest_dirs.is_empty() {
            sqlx::query(
                "DELETE FROM security_scoped_bookmarks
                 WHERE owner_sink_id = ? AND role = 'markdown_sink_dest'",
            )
            .bind(&sid)
            .execute(&self.pool)
            .await
            .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.delete_all_sink"))?;
            return Ok(());
        }

        let mut keys = Vec::with_capacity(dest_dirs.len());
        for p in dest_dirs {
            keys.push(markdown_sink_dest_logical_path(p)?);
        }
        let key_set: HashSet<&str> = keys.iter().map(String::as_str).collect();

        let existing = self.list_markdown_sink_dest_rows(sink_id).await?;
        for row in existing {
            if !key_set.contains(row.logical_path.as_str()) {
                sqlx::query(
                    "DELETE FROM security_scoped_bookmarks
                     WHERE owner_sink_id = ? AND role = 'markdown_sink_dest'
                       AND logical_path = ?",
                )
                .bind(&sid)
                .bind(&row.logical_path)
                .execute(&self.pool)
                .await
                .map_err(|e| {
                    DbError::classify_sqlx(e, "security_scoped_bookmarks.delete_stale_sink")
                })?;
            }
        }

        let now = chrono::Utc::now().to_rfc3339();
        for path in dest_dirs {
            let logical = markdown_sink_dest_logical_path(path)?;
            let count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM security_scoped_bookmarks
                 WHERE owner_sink_id = ? AND role = 'markdown_sink_dest'
                   AND logical_path = ?",
            )
            .bind(&sid)
            .bind(&logical)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.count_sink"))?;

            if count == 0 {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r#"INSERT INTO security_scoped_bookmarks
                        (id, owner_source_id, owner_sink_id, role, logical_path,
                         bookmark_blob, meta_json, created_at, updated_at)
                       VALUES (?, NULL, ?, 'markdown_sink_dest', ?, NULL, NULL, ?, ?)"#,
                )
                .bind(&id)
                .bind(&sid)
                .bind(&logical)
                .bind(&now)
                .bind(&now)
                .execute(&self.pool)
                .await
                .map_err(|e| DbError::classify_sqlx(e, "security_scoped_bookmarks.insert_sink"))?;
            }
        }

        Ok(())
    }
}

fn row_to_scan_row(row: sqlx::sqlite::SqliteRow) -> DbResult<LocalGitScanBookmarkRow> {
    Ok(LocalGitScanBookmarkRow {
        logical_path: row.try_get("logical_path")?,
        bookmark_blob: row.try_get("bookmark_blob")?,
    })
}

fn row_to_sink_dest_row(row: sqlx::sqlite::SqliteRow) -> DbResult<MarkdownSinkDestBookmarkRow> {
    Ok(MarkdownSinkDestBookmarkRow {
        logical_path: row.try_get("logical_path")?,
        bookmark_blob: row.try_get("bookmark_blob")?,
    })
}

//! Approved local git repositories. Keyed on absolute path; `source_id`
//! is carried as an FK so deleting a `LocalGit` source removes every
//! approved repo under it in one cascade.

use dayseam_core::{LocalRepo, SourceId};
use sqlx::{Row, SqlitePool};

use crate::error::{DbError, DbResult};

use super::helpers::parse_rfc3339;

#[derive(Clone)]
pub struct LocalRepoRepo {
    pool: SqlitePool,
}

impl LocalRepoRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert-or-update on `path`. Rescans never remove an existing row;
    /// they refresh the label / source_id / discovered_at metadata so
    /// user edits survive re-scans.
    ///
    /// `is_private` is **not** refreshed on conflict — it is owned by
    /// the user (via `set_is_private`), and discovery has no
    /// ground-truth for it (DAY-72 CORR-addendum-02). Every production
    /// caller of `upsert` (the `upsert_discovered_repos` path in the
    /// IPC layer) constructs rows with `is_private: false`; without
    /// this carve-out, every rescan silently un-redacts the private
    /// repos a user had marked — with no UI signal, which is the
    /// DAY-71 shape of bug this review addendum hunts for.
    pub async fn upsert(&self, source_id: &SourceId, repo: &LocalRepo) -> DbResult<()> {
        let path_str = path_as_str(&repo.path)?;
        let is_private = if repo.is_private { 1_i64 } else { 0_i64 };
        sqlx::query(
            "INSERT INTO local_repos (path, source_id, label, is_private, discovered_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(path) DO UPDATE SET
                source_id = excluded.source_id,
                label = excluded.label,
                discovered_at = excluded.discovered_at",
        )
        .bind(path_str)
        .bind(source_id.to_string())
        .bind(&repo.label)
        .bind(is_private)
        .bind(repo.discovered_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::classify_sqlx(e, "local_repos.upsert"))?;
        Ok(())
    }

    /// Look up a single approved repo by its absolute path. Used by
    /// the IPC layer to return the freshly-mutated row from
    /// `local_repos_set_private` without needing to re-list every
    /// repo for the parent source.
    pub async fn get(&self, path: &std::path::Path) -> DbResult<Option<LocalRepo>> {
        let row = sqlx::query(
            "SELECT path, label, is_private, discovered_at
             FROM local_repos WHERE path = ?",
        )
        .bind(path_as_str(path)?)
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_local_repo).transpose()
    }

    pub async fn list_for_source(&self, source_id: &SourceId) -> DbResult<Vec<LocalRepo>> {
        let rows = sqlx::query(
            "SELECT path, label, is_private, discovered_at
             FROM local_repos WHERE source_id = ? ORDER BY path ASC",
        )
        .bind(source_id.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_local_repo).collect()
    }

    pub async fn set_is_private(&self, path: &std::path::Path, is_private: bool) -> DbResult<()> {
        let v = if is_private { 1_i64 } else { 0_i64 };
        sqlx::query("UPDATE local_repos SET is_private = ? WHERE path = ?")
            .bind(v)
            .bind(path_as_str(path)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete(&self, path: &std::path::Path) -> DbResult<()> {
        sqlx::query("DELETE FROM local_repos WHERE path = ?")
            .bind(path_as_str(path)?)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_local_repo(row: sqlx::sqlite::SqliteRow) -> DbResult<LocalRepo> {
    let path: String = row.try_get("path")?;
    let is_private_int: i64 = row.try_get("is_private")?;
    let discovered_str: String = row.try_get("discovered_at")?;
    Ok(LocalRepo {
        path: std::path::PathBuf::from(path),
        label: row.try_get("label")?,
        is_private: is_private_int != 0,
        discovered_at: parse_rfc3339(&discovered_str, "local_repos.discovered_at")?,
    })
}

fn path_as_str(path: &std::path::Path) -> DbResult<String> {
    path.to_str()
        .map(String::from)
        .ok_or_else(|| DbError::InvalidData {
            column: "local_repos.path".into(),
            message: format!("path is not valid UTF-8: {path:?}"),
        })
}

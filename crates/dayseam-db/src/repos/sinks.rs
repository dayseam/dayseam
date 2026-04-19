//! Configured sinks — the write destinations the orchestrator dispatches
//! rendered reports to. One row per configured sink; parallel shape to
//! [`crate::SourceRepo`]. The JSON blob in `config_json` carries the
//! per-kind settings (dest directories, frontmatter toggle, …) and the
//! `kind` column picks the adapter `dayseam-orchestrator` dispatches
//! against.

use chrono::{DateTime, Utc};
use dayseam_core::{Sink, SinkConfig, SinkKind};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{DbError, DbResult};

use super::helpers::{parse_rfc3339, sink_kind_from_db, sink_kind_to_db};

#[derive(Clone)]
pub struct SinkRepo {
    pool: SqlitePool,
}

impl SinkRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, sink: &Sink) -> DbResult<()> {
        let kind = sink_kind_to_db(&sink.kind);
        let config = serde_json::to_string(&sink.config)?;
        sqlx::query(
            "INSERT INTO sinks (id, kind, label, config_json, created_at, last_write_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(sink.id.to_string())
        .bind(kind)
        .bind(&sink.label)
        .bind(config)
        .bind(sink.created_at.to_rfc3339())
        .bind(sink.last_write_at.map(|t| t.to_rfc3339()))
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::classify_sqlx(e, "sinks.insert"))?;
        Ok(())
    }

    pub async fn get(&self, id: &Uuid) -> DbResult<Option<Sink>> {
        let row = sqlx::query(
            "SELECT id, kind, label, config_json, created_at, last_write_at
             FROM sinks WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_sink).transpose()
    }

    pub async fn list(&self) -> DbResult<Vec<Sink>> {
        let rows = sqlx::query(
            "SELECT id, kind, label, config_json, created_at, last_write_at
             FROM sinks ORDER BY created_at ASC",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_sink).collect()
    }

    pub async fn delete(&self, id: &Uuid) -> DbResult<()> {
        sqlx::query("DELETE FROM sinks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Bump `last_write_at` to `at`. Called by the orchestrator after a
    /// successful [`dayseam_core::WriteReceipt`] so the Task 7 "recent
    /// sinks" sort key reflects real usage.
    pub async fn touch_last_write(&self, id: &Uuid, at: DateTime<Utc>) -> DbResult<()> {
        sqlx::query("UPDATE sinks SET last_write_at = ? WHERE id = ?")
            .bind(at.to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

fn row_to_sink(row: sqlx::sqlite::SqliteRow) -> DbResult<Sink> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str).map_err(|e| DbError::InvalidData {
        column: "sinks.id".into(),
        message: e.to_string(),
    })?;

    let kind_str: String = row.try_get("kind")?;
    let kind: SinkKind = sink_kind_from_db(&kind_str)?;

    let config_json: String = row.try_get("config_json")?;
    let config: SinkConfig = serde_json::from_str(&config_json)?;

    let created_at_str: String = row.try_get("created_at")?;
    let created_at = parse_rfc3339(&created_at_str, "sinks.created_at")?;

    let last_write_at: Option<String> = row.try_get("last_write_at")?;
    let last_write_at = match last_write_at {
        Some(s) => Some(parse_rfc3339(&s, "sinks.last_write_at")?),
        None => None,
    };

    Ok(Sink {
        id,
        kind,
        label: row.try_get("label")?,
        config,
        created_at,
        last_write_at,
    })
}

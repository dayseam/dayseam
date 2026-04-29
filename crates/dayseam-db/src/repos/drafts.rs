//! Report drafts. Each row is one rendered report for one date. Drafts
//! are append-only; regenerating a report creates a new row so the user
//! always has history, and retention sweeps prune the oldest rows.
//!
//! ## DAY-188 H2 (audit follow-up): `sync_run_id` linkage
//!
//! Migration 0002 added `report_drafts.sync_run_id` with the literal
//! intent "new drafts always carry a sync_run_id" — but the previous
//! `DraftRepo::insert` shape never bound the column. Every draft
//! ever written by v0.10 had `sync_run_id = NULL`, which silently
//! defeats every future join `report_drafts ⋈ sync_runs` (retention
//! reports, "what failed?" telemetry, future per-source rerun UI).
//!
//! The fix is to require a `Option<&RunId>` on insert. The production
//! orchestrator path passes `Some(&run_id)`; tests that don't care
//! about the linkage pass `None`. The optional-ness is preserved so
//! a future caller (e.g. an import-from-markdown path that has no
//! originating sync run) can still land a draft cleanly.

use chrono::{DateTime, Utc};
use dayseam_core::{ReportDraft, RunId};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::error::{DbError, DbResult};

use super::helpers::parse_rfc3339;

#[derive(Clone)]
pub struct DraftRepo {
    pool: SqlitePool,
}

impl DraftRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Persist `draft`, recording its originating `sync_run_id` so
    /// future joins between `report_drafts` and `sync_runs` work.
    /// Pass `None` only when the draft genuinely has no originating
    /// run (the historic test seeders that bypass the orchestrator).
    pub async fn insert(&self, draft: &ReportDraft, sync_run_id: Option<&RunId>) -> DbResult<()> {
        let sections = serde_json::to_string(&draft.sections)?;
        let evidence = serde_json::to_string(&draft.evidence)?;
        let per_source_state = serde_json::to_string(&draft.per_source_state)?;
        let verbose = if draft.verbose_mode { 1_i64 } else { 0_i64 };
        sqlx::query(
            "INSERT INTO report_drafts
                (id, date, template_id, template_version, sections_json, evidence_json,
                 per_source_state_json, verbose_mode, generated_at, sync_run_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(draft.id.to_string())
        .bind(draft.date.format("%Y-%m-%d").to_string())
        .bind(&draft.template_id)
        .bind(&draft.template_version)
        .bind(sections)
        .bind(evidence)
        .bind(per_source_state)
        .bind(verbose)
        .bind(draft.generated_at.to_rfc3339())
        .bind(sync_run_id.map(|r| r.to_string()))
        .execute(&self.pool)
        .await
        .map_err(|e| DbError::classify_sqlx(e, "report_drafts.insert"))?;
        Ok(())
    }

    /// Read back the `sync_run_id` for a previously-inserted draft.
    /// Returns `Ok(None)` either when no draft exists for `id` or
    /// when the existing draft was inserted before this column was
    /// wired (legacy v0.10 rows). Used by the regression test for
    /// DAY-188 H2 and by future retention queries that join drafts
    /// onto sync_runs.
    pub async fn sync_run_id_for(&self, id: &Uuid) -> DbResult<Option<RunId>> {
        let row: Option<(Option<String>,)> =
            sqlx::query_as("SELECT sync_run_id FROM report_drafts WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await?;
        match row {
            Some((Some(s),)) => Ok(Some(RunId(Uuid::parse_str(&s).map_err(|e| {
                DbError::InvalidData {
                    column: "report_drafts.sync_run_id".into(),
                    message: e.to_string(),
                }
            })?))),
            _ => Ok(None),
        }
    }

    pub async fn get(&self, id: &Uuid) -> DbResult<Option<ReportDraft>> {
        let row = sqlx::query(
            "SELECT id, date, template_id, template_version, sections_json, evidence_json,
                    per_source_state_json, verbose_mode, generated_at
             FROM report_drafts WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(row_to_draft).transpose()
    }

    pub async fn list_recent(&self, limit: u32) -> DbResult<Vec<ReportDraft>> {
        let rows = sqlx::query(
            "SELECT id, date, template_id, template_version, sections_json, evidence_json,
                    per_source_state_json, verbose_mode, generated_at
             FROM report_drafts
             ORDER BY generated_at DESC
             LIMIT ?",
        )
        .bind(limit as i64)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(row_to_draft).collect()
    }

    pub async fn prune_older_than(&self, cutoff: DateTime<Utc>) -> DbResult<u64> {
        let res = sqlx::query("DELETE FROM report_drafts WHERE generated_at < ?")
            .bind(cutoff.to_rfc3339())
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}

fn row_to_draft(row: sqlx::sqlite::SqliteRow) -> DbResult<ReportDraft> {
    let id_str: String = row.try_get("id")?;
    let id = Uuid::parse_str(&id_str).map_err(|e| DbError::InvalidData {
        column: "report_drafts.id".into(),
        message: e.to_string(),
    })?;
    let date_str: String = row.try_get("date")?;
    let date = chrono::NaiveDate::parse_from_str(&date_str, "%Y-%m-%d").map_err(|e| {
        DbError::InvalidData {
            column: "report_drafts.date".into(),
            message: e.to_string(),
        }
    })?;
    let sections_json: String = row.try_get("sections_json")?;
    let sections = serde_json::from_str(&sections_json)?;
    let evidence_json: String = row.try_get("evidence_json")?;
    let evidence = serde_json::from_str(&evidence_json)?;
    let per_src_json: String = row.try_get("per_source_state_json")?;
    let per_source_state = serde_json::from_str(&per_src_json)?;
    let verbose_int: i64 = row.try_get("verbose_mode")?;
    let generated_str: String = row.try_get("generated_at")?;
    let generated_at = parse_rfc3339(&generated_str, "report_drafts.generated_at")?;
    Ok(ReportDraft {
        id,
        date,
        template_id: row.try_get("template_id")?,
        template_version: row.try_get("template_version")?,
        sections,
        evidence,
        per_source_state,
        verbose_mode: verbose_int != 0,
        generated_at,
    })
}

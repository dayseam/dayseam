-- Sinks — the write-only destinations a rendered `ReportDraft` can be
-- dispatched to. Parallel shape to `sources`: the `config_json` blob
-- carries the per-kind settings and the schema key (`kind`) picks the
-- adapter the orchestrator dispatches to.
--
-- `last_write_at` is updated lazily by the orchestrator after a
-- successful `save_report` run so the Task 7 setup sidebar can sort
-- "recently used" sinks to the top of the save dialog.
CREATE TABLE sinks (
  id TEXT PRIMARY KEY,
  kind TEXT NOT NULL,
  label TEXT NOT NULL,
  config_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  last_write_at TEXT
);

CREATE INDEX idx_sinks_kind ON sinks(kind);

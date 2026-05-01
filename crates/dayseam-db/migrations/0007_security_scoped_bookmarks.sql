-- MAS-4a — Security-scoped bookmark persistence for App Sandbox filesystem access.
--
-- One row per user-granted directory: either a Local Git scan root
-- (`sources.config.LocalGit.scan_roots[]`) or a Markdown-file sink destination
-- (`sinks.config.MarkdownFile.dest_dirs[]`). The `logical_path` string must
-- match the corresponding path in that JSON so **MAS-4c/4d** can associate
-- bookmarks with config rows; **MAS-4b** fills `bookmark_blob` from the macOS
-- security-scoped bookmark APIs after `dialog.open`.
--
-- `meta_json` holds §9.4 metadata (canonical path, symlink policy) as JSON.
-- `bookmark_blob` may be NULL until the user completes a sandbox picker flow.
--
-- Normative design: docs/design/2026-phase-5-mas-architecture.md §9.

CREATE TABLE security_scoped_bookmarks (
  id TEXT PRIMARY KEY,
  owner_source_id TEXT REFERENCES sources(id) ON DELETE CASCADE,
  owner_sink_id TEXT REFERENCES sinks(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('local_git_scan_root', 'markdown_sink_dest')),
  logical_path TEXT NOT NULL,
  bookmark_blob BLOB,
  meta_json TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK (
    (owner_source_id IS NOT NULL AND owner_sink_id IS NULL)
    OR (owner_source_id IS NULL AND owner_sink_id IS NOT NULL)
  ),
  CHECK (
    (owner_source_id IS NOT NULL AND role = 'local_git_scan_root')
    OR (owner_sink_id IS NOT NULL AND role = 'markdown_sink_dest')
  )
);

CREATE INDEX idx_security_scoped_bookmarks_source
  ON security_scoped_bookmarks(owner_source_id)
  WHERE owner_source_id IS NOT NULL;

CREATE INDEX idx_security_scoped_bookmarks_sink
  ON security_scoped_bookmarks(owner_sink_id)
  WHERE owner_sink_id IS NOT NULL;

CREATE UNIQUE INDEX uq_security_scoped_bookmarks_source_path
  ON security_scoped_bookmarks(owner_source_id, logical_path)
  WHERE owner_source_id IS NOT NULL;

CREATE UNIQUE INDEX uq_security_scoped_bookmarks_sink_path
  ON security_scoped_bookmarks(owner_sink_id, logical_path)
  WHERE owner_sink_id IS NOT NULL;

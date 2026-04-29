-- Dayseam DAY-188 audit follow-up: two partial UNIQUE indexes that
-- close a pair of dedup races caught in the cluster D scan.

-- 1. `source_identities` NULL-dedup gap (Data M1).
--
-- The original `UNIQUE(person_id, source_id, kind, external_actor_id)`
-- constraint from migration 0003 treats `source_id IS NULL` as
-- "different from every other NULL", which is correct ANSI SQL but
-- defeats the dedup intent for source-agnostic identities (a bare
-- `GitEmail` carries no `source_id`). The DAO's `INSERT OR IGNORE`
-- silently appends a duplicate row on every re-run.
--
-- Latent today (every production caller passes `Some(_)`), but ready
-- to bite the moment a bare-GitEmail backfill lands. The fix is a
-- partial UNIQUE index that substitutes a sentinel (empty string)
-- for NULL, so two source-agnostic identities for the same
-- `(person_id, kind, external_actor_id)` collide as expected.
CREATE UNIQUE INDEX idx_source_identities_dedup_with_null_source
  ON source_identities (person_id, IFNULL(source_id, ''), kind, external_actor_id);

-- 2. Outlook duplicate-add race (Data M2).
--
-- The desktop `outlook_sources_add` IPC does a "find by tenant_id +
-- upn" probe followed by an `INSERT INTO sources` outside any
-- transaction. Two concurrent invocations for the same calendar both
-- observe `find = None` and both insert, leaving the user with two
-- visible rows for one Outlook account.
--
-- The schema-level fix is a partial UNIQUE index keyed on
-- `(LOWER(tenant_id), LOWER(user_principal_name))` for `kind =
-- 'Outlook'` rows. SQLite's JSON1 functions are bundled by default
-- and `json_extract` is deterministic, so this index is well-formed
-- and cheap. Once it's in place, the second `INSERT` fails with a
-- UNIQUE constraint and the IPC layer surfaces the standard
-- `db.constraint_violation` error code instead of letting both
-- writes through.
--
-- Note on case-sensitivity: tenant ids are GUIDs (case-insensitive
-- lookup is correct for upstream Microsoft Graph behaviour) and
-- user principal names ("upn") are RFC-5321-style local-part@domain
-- where the domain is case-insensitive and Microsoft normalises the
-- whole string to lower-case in their tenants. Lower-casing both
-- sides matches that contract without forcing the IPC layer to
-- normalise on the way in.
CREATE UNIQUE INDEX idx_sources_outlook_unique_account
  ON sources (
    LOWER(json_extract(config_json, '$.Outlook.tenant_id')),
    LOWER(json_extract(config_json, '$.Outlook.user_principal_name'))
  )
  WHERE kind = 'Outlook'
    AND json_extract(config_json, '$.Outlook.tenant_id') IS NOT NULL
    AND json_extract(config_json, '$.Outlook.user_principal_name') IS NOT NULL;

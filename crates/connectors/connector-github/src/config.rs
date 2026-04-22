//! Per-source GitHub configuration carried on a
//! [`dayseam_core::SourceConfig::GitHub`] row once it has been parsed
//! into a [`Url`].
//!
//! The core-types row holds `api_base_url` as a `String` (for the same
//! `PartialEq` + `Eq` + serde-round-trip reasons as every other
//! `SourceConfig` variant); this crate parses that string into a
//! stricter [`Url`] the moment the source is hydrated, so every
//! downstream call in `auth.rs` / `connector.rs` / (DAY-96) `walk.rs`
//! can assume a well-formed URL with a scheme and a host.
//!
//! The default for the `api_base_url` field is `https://api.github.com`
//! (github.com). GitHub Enterprise Server users pass
//! `https://<host>/api/v3` at add-source time; v0.4 tests the github.com
//! path end-to-end, the Enterprise code path is exercised by unit tests
//! but not hit by an integration test against a real Enterprise
//! instance.

use url::Url;

/// Canonical github.com API root. Used as the default when a caller
/// constructs a [`GithubConfig`] without supplying an explicit base.
pub const GITHUB_COM_API_BASE_URL: &str = "https://api.github.com";

/// Typed view of a [`dayseam_core::SourceConfig::GitHub`] row.
///
/// Constructed once at hydration time (DAY-99 IPC / startup backfill)
/// and threaded into [`crate::connector::GithubConnector::new`]. Unlike
/// Jira's config, GitHub's config carries no email — the
/// `Authorization: Bearer <token>` header is self-identifying, and the
/// `{ id, login }` pair GitHub echoes back from `GET /user` is what
/// [`crate::auth::list_identities`] persists for self-filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubConfig {
    /// Root URL of the GitHub REST API. `https://api.github.com/` for
    /// github.com tenants, `https://<host>/api/v3/` for GitHub
    /// Enterprise Server. Stored with a trailing slash so [`Url::join`]
    /// does not silently drop the last path segment when appending
    /// `user`, `search/issues`, …
    pub api_base_url: Url,
}

impl GithubConfig {
    /// Construct a [`GithubConfig`] from the raw `SourceConfig::GitHub`
    /// `api_base_url` string. Ensures the URL carries a trailing slash
    /// so every caller can use [`Url::join`] verbatim.
    pub fn from_raw(api_base_url: &str) -> Result<Self, url::ParseError> {
        let with_slash = if api_base_url.ends_with('/') {
            api_base_url.to_string()
        } else {
            format!("{api_base_url}/")
        };
        Ok(Self {
            api_base_url: Url::parse(&with_slash)?,
        })
    }

    /// Shorthand for the github.com default base URL — handy in tests
    /// and in the DAY-99 Add-Source dialog's "Advanced" override
    /// default.
    pub fn github_com() -> Self {
        Self::from_raw(GITHUB_COM_API_BASE_URL)
            .expect("github.com API base URL parses as a well-formed URL")
    }
}

impl Default for GithubConfig {
    fn default() -> Self {
        Self::github_com()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_pads_trailing_slash_so_url_join_is_safe() {
        let cfg = GithubConfig::from_raw("https://api.github.com").unwrap();
        // `Url::join` on a URL without a trailing slash would drop the
        // last path segment; the padding here defends against that
        // silent footgun.
        let joined = cfg.api_base_url.join("user").unwrap();
        assert_eq!(joined.as_str(), "https://api.github.com/user");
    }

    #[test]
    fn from_raw_is_idempotent_across_trailing_slashes() {
        let a = GithubConfig::from_raw("https://api.github.com").unwrap();
        let b = GithubConfig::from_raw("https://api.github.com/").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn from_raw_accepts_enterprise_server_path() {
        // GitHub Enterprise Server's API root lives under `/api/v3/`.
        // A caller passing the Enterprise URL must land on the same
        // well-formed shape the github.com path produces — trailing
        // slash and all — so downstream `Url::join` calls work
        // identically on both flavours.
        let cfg = GithubConfig::from_raw("https://ghe.example.com/api/v3").unwrap();
        let joined = cfg.api_base_url.join("user").unwrap();
        assert_eq!(joined.as_str(), "https://ghe.example.com/api/v3/user");
    }

    #[test]
    fn from_raw_rejects_malformed_urls() {
        assert!(GithubConfig::from_raw("not a url").is_err());
    }

    #[test]
    fn github_com_helper_matches_from_raw_default() {
        assert_eq!(
            GithubConfig::github_com(),
            GithubConfig::from_raw("https://api.github.com/").unwrap(),
        );
    }
}

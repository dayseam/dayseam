//! Per-source Confluence configuration carried on a
//! [`dayseam_core::SourceConfig::Confluence`] row once it has been
//! parsed into a [`Url`].
//!
//! The core-types row holds `workspace_url` as a `String` (so
//! `SourceConfig` can stay `PartialEq` + `Eq`, which `url::Url` is
//! not); this crate parses that string into a stricter [`Url`] the
//! moment the source is hydrated, so every downstream call in
//! `auth.rs` / `connector.rs` / (DAY-80) `walk.rs` can assume a
//! well-formed URL with a scheme and a host.
//!
//! Unlike [`connector_jira::JiraConfig`], this struct does not carry
//! an `email` — the paired Jira source row (or a dedicated Confluence
//! secret in the separate-credential follow-up) carries that, and
//! the IPC layer rebuilds a [`connectors_sdk::BasicAuth`] from
//! `(email, keychain_secret_ref)` on demand.
//!
//! [`connector_jira::JiraConfig`]: https://docs.rs/connector-jira

use url::Url;

/// Typed view of a [`dayseam_core::SourceConfig::Confluence`] row.
///
/// Constructed once at hydration time (DAY-82 IPC / startup backfill)
/// and threaded into [`crate::connector::ConfluenceConnector::new`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluenceConfig {
    /// Canonical Confluence Cloud workspace URL, e.g.
    /// `https://acme.atlassian.net/`. The Confluence connector joins
    /// `/wiki/rest/api/…` and `/wiki/api/v2/…` onto it; the auth
    /// probe (`GET /rest/api/3/myself`) shares the Jira endpoint
    /// because a single Atlassian Cloud credential authenticates both
    /// products. Stored with a trailing slash so [`Url::join`] does
    /// not silently drop the last path segment when appending
    /// `wiki/rest/api/…`.
    pub workspace_url: Url,
}

impl ConfluenceConfig {
    /// Construct a [`ConfluenceConfig`] from the raw
    /// [`dayseam_core::SourceConfig::Confluence`] field. Ensures
    /// `workspace_url` carries a trailing slash so every caller can
    /// use [`Url::join`] verbatim against `wiki/rest/api/…` or
    /// `wiki/api/v2/…`.
    pub fn from_raw(workspace_url: &str) -> Result<Self, url::ParseError> {
        let with_slash = if workspace_url.ends_with('/') {
            workspace_url.to_string()
        } else {
            format!("{workspace_url}/")
        };
        Ok(Self {
            workspace_url: Url::parse(&with_slash)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_pads_trailing_slash_so_url_join_is_safe() {
        let cfg = ConfluenceConfig::from_raw("https://acme.atlassian.net").unwrap();
        // `Url::join` on a URL without a trailing slash would drop
        // the last path segment; the padding here defends against
        // that silent footgun the DAY-80 walker will rely on when
        // joining `wiki/rest/api/content/search`.
        let joined = cfg
            .workspace_url
            .join("wiki/rest/api/content/search")
            .unwrap();
        assert_eq!(
            joined.as_str(),
            "https://acme.atlassian.net/wiki/rest/api/content/search"
        );
    }

    #[test]
    fn from_raw_is_idempotent_across_trailing_slashes() {
        let a = ConfluenceConfig::from_raw("https://acme.atlassian.net").unwrap();
        let b = ConfluenceConfig::from_raw("https://acme.atlassian.net/").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn from_raw_rejects_malformed_urls() {
        assert!(ConfluenceConfig::from_raw("not a url").is_err());
    }
}

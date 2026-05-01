//! Keychain **service** name strings per distribution SKU (**MAS-5b2**).
//!
//! Direct-download builds use the historical `dayseam.{connector}` names.
//! Mac App Store builds (`--features mas`) prefix with `dayseam.mas.` so
//! Keychain Access labels stay unambiguous when both SKUs are installed.
//! Account shapes are unchanged; bundle id / code signing remains the
//! primary isolation mechanism (architecture §12).

#[cfg(feature = "mas")]
macro_rules! mas_prefixed {
    ($suffix:literal) => {
        concat!("dayseam.mas.", $suffix)
    };
}

#[cfg(feature = "mas")]
pub const GITLAB_KEYCHAIN_SERVICE: &str = mas_prefixed!("gitlab");
#[cfg(not(feature = "mas"))]
pub const GITLAB_KEYCHAIN_SERVICE: &str = "dayseam.gitlab";

#[cfg(feature = "mas")]
pub const GITHUB_KEYCHAIN_SERVICE: &str = mas_prefixed!("github");
#[cfg(not(feature = "mas"))]
pub const GITHUB_KEYCHAIN_SERVICE: &str = "dayseam.github";

#[cfg(feature = "mas")]
pub const ATLASSIAN_KEYCHAIN_SERVICE: &str = mas_prefixed!("atlassian");
#[cfg(not(feature = "mas"))]
pub const ATLASSIAN_KEYCHAIN_SERVICE: &str = "dayseam.atlassian";

#[cfg(feature = "mas")]
pub const OUTLOOK_KEYCHAIN_SERVICE: &str = mas_prefixed!("outlook");
#[cfg(not(feature = "mas"))]
pub const OUTLOOK_KEYCHAIN_SERVICE: &str = "dayseam.outlook";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mas_build_prefixes_all_connector_services() {
        #[cfg(feature = "mas")]
        {
            for svc in [
                GITLAB_KEYCHAIN_SERVICE,
                GITHUB_KEYCHAIN_SERVICE,
                ATLASSIAN_KEYCHAIN_SERVICE,
                OUTLOOK_KEYCHAIN_SERVICE,
            ] {
                assert!(
                    svc.starts_with("dayseam.mas."),
                    "expected mas service prefix, got {svc:?}"
                );
            }
        }
        #[cfg(not(feature = "mas"))]
        {
            assert_eq!(GITLAB_KEYCHAIN_SERVICE, "dayseam.gitlab");
            assert_eq!(GITHUB_KEYCHAIN_SERVICE, "dayseam.github");
            assert_eq!(ATLASSIAN_KEYCHAIN_SERVICE, "dayseam.atlassian");
            assert_eq!(OUTLOOK_KEYCHAIN_SERVICE, "dayseam.outlook");
        }
    }

    #[test]
    fn connector_services_are_pairwise_distinct() {
        let svcs = [
            GITLAB_KEYCHAIN_SERVICE,
            GITHUB_KEYCHAIN_SERVICE,
            ATLASSIAN_KEYCHAIN_SERVICE,
            OUTLOOK_KEYCHAIN_SERVICE,
        ];
        for i in 0..svcs.len() {
            for j in i + 1..svcs.len() {
                assert_ne!(
                    svcs[i], svcs[j],
                    "duplicate keychain service: {:?}",
                    svcs[i]
                );
            }
        }
    }
}

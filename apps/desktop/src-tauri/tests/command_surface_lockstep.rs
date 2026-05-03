//! `build.rs` and `src/main.rs` duplicate the production command allow-list
//! beside `ipc::commands::PROD_COMMANDS`. `DEV_COMMANDS` in `build.rs` is the
//! full dev surface; `ipc::commands::DEV_COMMANDS` lists only the commands
//! that exist beyond production (see `capabilities.dev.template.json`).
//! Drift breaks tauri-build permission generation or runtime `invoke`.
//!
//! Capability parity lives in `tests/capabilities.rs`; this module
//! catches the other two manual lists.

use dayseam_desktop::ipc::commands::{DEV_COMMANDS, PROD_COMMANDS};

const BUILD_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/build.rs"));
const MAIN_RS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));

fn slice_between_const<'a>(src: &'a str, const_name: &str) -> &'a str {
    let marker = format!("const {const_name}: &[&str] = &");
    let start = src
        .find(&marker)
        .unwrap_or_else(|| panic!("build.rs: missing `{marker}`"));
    let rest = &src[start + marker.len()..];
    let open = rest
        .find('[')
        .expect("build.rs: expected `[` after command const");
    let from = &rest[open + 1..];
    let end = from
        .find("];")
        .expect("build.rs: expected `];` closing command array");
    &from[..end]
}

fn quoted_commands(body: &str) -> Vec<String> {
    body.lines()
        .filter_map(|line| {
            let t = line.trim();
            let rest = t.strip_prefix('"')?;
            let (name, _) = rest.split_once('"')?;
            if name.is_empty() {
                return None;
            }
            Some(name.to_string())
        })
        .collect()
}

#[test]
fn prod_commands_match_build_rs() {
    let body = slice_between_const(BUILD_RS, "PROD_COMMANDS");
    let from_build = quoted_commands(body);
    let from_src: Vec<String> = PROD_COMMANDS.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(
        from_build, from_src,
        "build.rs `PROD_COMMANDS` drifted from `ipc::commands::PROD_COMMANDS`"
    );
}

#[test]
fn dev_commands_slice_in_build_rs_extends_prod() {
    let body = slice_between_const(BUILD_RS, "DEV_COMMANDS");
    let from_build = quoted_commands(body);
    let mut expected: Vec<String> = PROD_COMMANDS.iter().map(|s| (*s).to_string()).collect();
    for cmd in DEV_COMMANDS {
        expected.push((*cmd).to_string());
    }
    assert_eq!(
        from_build, expected,
        "build.rs `DEV_COMMANDS` should be production commands plus `ipc::commands::DEV_COMMANDS` extras, in that order"
    );
}

fn rust_path_fragment(cmd: &str) -> &'static str {
    if cmd.starts_with("atlassian_") {
        "atlassian::"
    } else if cmd.starts_with("github_") {
        "github::"
    } else if cmd.starts_with("scheduler_") {
        "scheduler::"
    } else if cmd.starts_with("oauth_") {
        "oauth::"
    } else if cmd.starts_with("outlook_") {
        "outlook::"
    } else {
        "commands::"
    }
}

#[test]
fn prod_commands_registered_in_main_invoke_handlers() {
    for cmd in PROD_COMMANDS {
        let prefix = rust_path_fragment(cmd);
        let needle = format!("{prefix}{cmd}");
        assert!(
            MAIN_RS.contains(&needle),
            "production command `{cmd}` not found as `{needle}` in main.rs `generate_handler!` lists"
        );
    }
}

#[test]
fn dev_only_commands_in_main_under_dev_feature() {
    for cmd in DEV_COMMANDS {
        let needle = format!("commands::{cmd}");
        assert!(
            MAIN_RS.contains(&needle),
            "dev-only command `{cmd}` missing from main.rs (expected `{needle}`)"
        );
    }
}

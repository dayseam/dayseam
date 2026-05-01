//! Cargo does not reliably treat `./migrations` as an input to the
//! `sqlx::migrate!` expansion in [`src/pool.rs`]. Per the Cargo book,
//! `cargo:rerun-if-changed` on a **directory** scans every file under it for
//! modifications — including **new** `*.sql` migrations — so this script reruns
//! and the library rebuilds whenever the schema folder changes (listing each
//! existing file alone would miss freshly added migrations whose neighbours'
//! mtimes are unchanged).

fn main() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    println!("cargo:rerun-if-changed={}", dir.display());
}

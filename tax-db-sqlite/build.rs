//! Embeds every `seeds/*.sql` script into the crate at build time.
//!
//! Generates `$OUT_DIR/seeds.rs`, a `SEEDS` table with one entry per script
//! in file-name order. No names are hard-coded here: the directory is scanned
//! on every build, and cargo re-runs this script when its contents change.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let seeds_dir = Path::new(&manifest_dir).join("seeds");

    // A directory path makes cargo rescan the whole directory, so adding or
    // removing a script triggers a rebuild, not just editing one.
    println!("cargo:rerun-if-changed={}", seeds_dir.display());

    let scripts = seed_scripts(&seeds_dir);
    let source = generate_source(&scripts);
    fs::write(Path::new(&out_dir).join("seeds.rs"), source).expect("write generated seeds.rs");
}

/// Every `*.sql` file directly inside `seeds_dir`, sorted by path.
fn seed_scripts(seeds_dir: &Path) -> Vec<PathBuf> {
    let entries = fs::read_dir(seeds_dir)
        .unwrap_or_else(|e| panic!("cannot read seeds directory '{}': {e}", seeds_dir.display()));

    let mut scripts: Vec<PathBuf> = entries
        .map(|entry| entry.expect("readable seeds directory entry").path())
        .filter(|path| path.is_file() && is_sql(path))
        .collect();
    scripts.sort();
    scripts
}

fn is_sql(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("sql"))
}

/// Renders the `SEEDS` table. `Seed` itself is defined in `src/seeds.rs`,
/// which `include!`s this output.
fn generate_source(scripts: &[PathBuf]) -> String {
    let mut source = String::from("const SEEDS: &[Seed] = &[\n");
    for path in scripts {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("seed file name is valid UTF-8");
        let path = path.to_str().expect("seed path is valid UTF-8");
        let entry = format!("    Seed {{ name: {name:?}, sql: include_str!({path:?}) }},\n");
        source.push_str(&entry);
    }
    source.push_str("];\n");
    source
}

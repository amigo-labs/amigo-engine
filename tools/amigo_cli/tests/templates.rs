//! Every `amigo new` template must produce a project that builds.
//!
//! Generated projects are only ever compiled by users, so a template that
//! stopped building went unnoticed until someone ran `amigo new`. This test
//! scaffolds one project per template against this checkout (`--path`) and
//! type-checks each of them as the standalone crate a user gets, with
//! warnings as errors.
//!
//! It compiles the whole engine in its own target directory, so it is
//! `#[ignore]`d in the default `cargo test` run. Run it with `just templates`:
//!
//! ```sh
//! cargo test -p amigo_cli --test templates -- --ignored --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use amigo_core::game_preset::project_templates;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root")
}

#[test]
#[ignore = "builds the engine for every template; run with `just templates`"]
fn every_template_builds() {
    let root = repo_root();
    let scratch = tempfile::tempdir().expect("temp dir");
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    // Shared by every project: they resolve the same engine with the same
    // lockfile and profile, so the engine is compiled once.
    let target_dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("templates");
    let mut failed = Vec::new();

    for template in project_templates() {
        let slug = template.slug();
        // Project names must be valid crate names.
        let project = format!("tpl_{}", slug.replace('-', "_"));
        // The slug is what `amigo list-templates` prints, so passing it here
        // also checks that every listed name is accepted.
        let output = Command::new(env!("CARGO_BIN_EXE_amigo"))
            .current_dir(scratch.path())
            .args(["new", &project, "--template", &slug, "--path"])
            .arg(&root)
            .output()
            .expect("run amigo new");
        assert!(
            output.status.success(),
            "`amigo new {project} --template {slug}` failed:\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );

        let dir = scratch.path().join(&project);
        // The engine's lockfile pins the dependency versions CI builds the
        // engine with; cargo adds the game crate and drops what it does not
        // use, instead of resolving whatever is newest today.
        std::fs::copy(root.join("Cargo.lock"), dir.join("Cargo.lock")).expect("copy Cargo.lock");

        eprintln!("--- checking template {slug}");
        let status = Command::new(&cargo)
            .current_dir(&dir)
            .args(["check", "--all-targets"])
            .env("CARGO_TARGET_DIR", &target_dir)
            .env("RUSTFLAGS", "-D warnings")
            .status()
            .expect("run cargo check");
        if !status.success() {
            failed.push(slug);
        }
    }

    assert!(
        failed.is_empty(),
        "projects generated from these templates do not build: {failed:?} \
         (cargo output above)"
    );
}

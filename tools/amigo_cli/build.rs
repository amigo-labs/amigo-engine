use std::process::Command;

/// Embed the engine git revision the CLI was built from, so `amigo new`
/// can pin generated projects to a known-good engine commit.
fn main() {
    let rev = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    println!("cargo:rustc-env=AMIGO_ENGINE_REV={rev}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}

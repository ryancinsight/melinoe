//! Build script to dynamically detect toolchain capabilities for the Melinoe crate.
//!
//! Specifically, this detects whether we are on a nightly toolchain or docs.rs
//! to determine if we should enable unstable features like `doc_cfg`.

use std::{env, process::Command};

/// Main entry point of the build script.
fn main() {
    println!("cargo:rustc-check-cfg=cfg(doc_cfg_active)");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=RUSTC");

    let is_nightly_feature = env::var_os("CARGO_FEATURE_NIGHTLY").is_some();
    let is_docsrs = env::var_os("CARGO_CFG_DOCSRS").is_some();

    if !is_nightly_feature && !is_docsrs {
        return;
    }

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let Ok(output) = Command::new(rustc).arg("-vV").output() else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let version = String::from_utf8_lossy(&output.stdout);
    let is_nightly_compiler = version
        .lines()
        .any(|line| match line.strip_prefix("release: ") {
            Some(release) => release.contains("nightly"),
            None => false,
        });

    if is_nightly_compiler || is_docsrs {
        println!("cargo:rustc-cfg=doc_cfg_active");
    }
}

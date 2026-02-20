//! xtask — Build automation for the uterx workspace.
//!
//! Usage: `cargo xtask <task>`
//!
//! Tasks:
//!   build-plugins  — Compile all plugins to WASM
//!   dist           — Build release binaries
//!   docs           — Build mdBook documentation

use std::process::Command;

fn main() {
    let task = std::env::args().nth(1);
    match task.as_deref() {
        Some("build-plugins") => build_plugins(),
        Some("dist") => dist(),
        Some("docs") => docs(),
        _ => {
            eprintln!(
                "Usage: cargo xtask <task>\n\nTasks:\n  build-plugins  Compile plugins to WASM\n  dist           Build release binaries\n  docs           Build documentation"
            );
            std::process::exit(1);
        }
    }
}

fn build_plugins() {
    let plugins = [
        "plugins/bluetooth",
        "plugins/mesh-chat",
        "plugins/midnight",
        "plugins/editor",
        "plugins/file-sharing",
        "plugins/network-tools",
        "plugins/ssh-tools",
        "plugins/converter",
    ];

    for plugin in &plugins {
        println!("Building plugin: {}", plugin);
        let status = Command::new("cargo")
            .args(["build", "--release", "--target", "wasm32-wasip1", "-p"])
            .arg(
                plugin
                    .rsplit('/')
                    .next()
                    .unwrap_or(plugin),
            )
            .status()
            .expect("failed to run cargo");
        if !status.success() {
            eprintln!("Failed to build plugin: {}", plugin);
            std::process::exit(1);
        }
    }
    println!("All plugins built successfully.");
}

fn dist() {
    println!("Building release binary...");
    let status = Command::new("cargo")
        .args(["build", "--release", "-p", "uterx"])
        .status()
        .expect("failed to run cargo");
    if !status.success() {
        std::process::exit(1);
    }
    println!("Release binary built.");
}

fn docs() {
    println!("Building documentation...");
    let status = Command::new("mdbook")
        .args(["build", "docs"])
        .status()
        .expect("failed to run mdbook — is it installed?");
    if !status.success() {
        std::process::exit(1);
    }
    println!("Documentation built.");
}

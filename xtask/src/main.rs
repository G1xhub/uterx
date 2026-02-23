//! xtask — Build automation for the uterx workspace.
//!
//! Usage: `cargo xtask <task>`
//!
//! Tasks:
//!   build-all      — Build app + plugins (WASM)
//!   build-plugins  — Compile all plugins to WASM
//!   dist           — Build release binaries
//!   docs           — Build mdBook documentation

use std::process::Command;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [task] if task == "build-all" => build_all(),
        [task] if task == "build-plugins" => build_plugins(),
        [task] if task == "dist" => dist(),
        [task] if task == "docs" => docs(),
        [cmd, sub] if cmd == "build" && sub == "all" => build_all(),
        [cmd, sub] if cmd == "build" && sub == "plugins" => build_plugins(),
        _ => {
            eprintln!(
                "Usage: cargo xtask <task>\n\nTasks:\n  build-all      Build app + plugins (WASM)\n  build-plugins  Compile plugins to WASM\n  dist           Build release binaries\n  docs           Build documentation\n\nAliases:\n  build all      Same as build-all\n  build plugins  Same as build-plugins"
            );
            std::process::exit(1);
        }
    }
}

fn build_all() {
    println!("Building app (uterx)...");
    let app_status = Command::new("cargo")
        .args(["build", "-p", "uterx"])
        .status()
        .expect("failed to run cargo build for app");
    if !app_status.success() {
        eprintln!("Failed to build app (uterx)");
        std::process::exit(1);
    }

    build_plugins();
}

fn build_plugins() {
    let plugins = [
        "plugins/uterxai",
        "plugins/quick-notes",
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

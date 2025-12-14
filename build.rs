//! Build script for mini-blockchain
//!
//! This automatically builds the web-ui frontend before compiling Rust.
//! The frontend is then embedded into the binary via rust-embed.

use std::path::Path;
use std::process::Command;

fn main() {
    // Only rebuild frontend if web-ui source files changed
    println!("cargo::rerun-if-changed=web-ui/src");
    println!("cargo::rerun-if-changed=web-ui/package.json");
    println!("cargo::rerun-if-changed=web-ui/svelte.config.js");
    println!("cargo::rerun-if-changed=web-ui/vite.config.ts");

    let web_ui_dir = Path::new("web-ui");

    // Check if web-ui directory exists
    if !web_ui_dir.exists() {
        println!("cargo::warning=web-ui directory not found, skipping frontend build");
        return;
    }

    // Check if node_modules exists, if not run npm install
    let node_modules = web_ui_dir.join("node_modules");
    if !node_modules.exists() {
        println!("cargo::warning=Installing web-ui dependencies...");
        let status = Command::new("npm")
            .arg("install")
            .current_dir(web_ui_dir)
            .status()
            .expect("Failed to run npm install");

        if !status.success() {
            panic!("npm install failed");
        }
    }

    // Build the frontend
    println!("cargo::warning=Building web-ui frontend...");
    let status = Command::new("npm")
        .arg("run")
        .arg("build")
        .current_dir(web_ui_dir)
        .status()
        .expect("Failed to run npm run build");

    if !status.success() {
        panic!("Frontend build failed! Run 'cd web-ui && npm run build' manually to see errors.");
    }

    println!("cargo::warning=Frontend build complete!");
}

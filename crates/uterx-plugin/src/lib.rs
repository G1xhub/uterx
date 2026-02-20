//! uterx-plugin — WASM plugin engine.
//!
//! Loads, manages, and runs WASM plugins using wasmtime.
//! Exposes host APIs (ui, io, net, fs, platform) to plugins
//! with a capability-based permission system.

pub mod host_api;
pub mod manifest;
pub mod manager;
pub mod runtime;

pub use manager::PluginManager;
pub use manifest::PluginManifest;
pub use runtime::PluginInstance;

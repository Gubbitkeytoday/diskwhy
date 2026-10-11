//! DiskWhy core — everything that is not UI: scanning, analysis, safety rules and Windows actions.
//! Shared by the desktop app (src-tauri) and the MCP server / CLI (crates/cli).

pub mod actions;
pub mod insights;
pub mod installed;
pub mod report;
pub mod safety;
pub mod scan;

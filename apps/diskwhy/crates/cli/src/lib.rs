//! DiskWhy CLI + MCP server. The binary is `diskwhy-cli`; see `diskwhy-cli --help`.
//!
//! * [`views`]  - pure result builders (Thai text + JSON) shared by the CLI and the MCP tools.
//! * [`engine`] - server state: last scan, background scan, safe Recycle Bin moves, whitelisted cleanups.
//! * [`mcp`]    - the MCP stdio server (rmcp) with tools and prompts.
//! * [`policy`] - the expert safety policy sent as MCP instructions / prompts.

pub mod cli;
pub mod engine;
pub mod fmt;
pub mod mcp;
pub mod policy;
pub mod views;

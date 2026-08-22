//! Daemon module - IPC server and handlers

pub mod cache;
// Persistence is an internal detail of the daemon subtree: no binary,
// integration test, or benchmark consumes it directly.
mod db;
pub mod handlers;
pub mod index;
pub mod protocol;
pub mod server;
mod status_policy;

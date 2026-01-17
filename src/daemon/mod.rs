//! Daemon module - IPC server and handlers

pub mod cache;
pub mod db;
#[cfg(feature = "debian")]
pub mod debian_index;
pub mod handlers;
pub mod index;
pub mod protocol;
pub mod server;

//! jdtui — a terminal UI for JDownloader 2, over the My.JDownloader API.
//!
//! The binary is a thin wrapper around these modules; they are public so the
//! screenshot example can draw real frames.

pub mod api;
pub mod app;
pub mod config;
pub mod model;
pub mod myjd;
pub mod poller;
pub mod ui;

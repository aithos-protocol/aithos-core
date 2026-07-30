//! One module per command (lot SPL-5): each module owns its clap `Args`
//! struct and its `run` body, moved verbatim from the historical
//! `main.rs`; `main()` is reduced to parse + dispatch. Shared surface
//! helpers live in [`common`].

pub mod common;

pub mod action;
pub mod approve;
pub mod edition_diff;
pub mod edition_merge;
pub mod edition_publish;
pub mod edition_verify;
pub mod folder_add;
pub mod grant;
pub mod grant_act;
pub mod header_open;
pub mod header_seal;
pub mod heartbeat;
pub mod inference;
pub mod init;
pub mod log_audit;
pub mod log_prove;
pub mod log_query;
pub mod log_show;
pub mod log_verify;
pub mod mandate_verify;
pub mod move_folder;
pub mod node_key;
pub mod oauth;
pub mod owner;
pub mod prove;
pub mod revoke;
pub mod section_add;
pub mod section_read;
pub mod section_read_agent;
pub mod status;
pub mod zone_show;

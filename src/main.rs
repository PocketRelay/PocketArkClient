#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]
#![warn(unused_crate_dependencies)]

use config::read_config_file;
use core::{api::create_http_client, api::read_client_identity, reqwest};
use hosts::HostEntryGuard;
use log::error;
use pocket_ark_client_shared as core;
use std::path::Path;
use ui::show_confirm;

use crate::ui::show_error;

pub mod config;
pub mod hosts;
pub mod patch;
pub mod servers;
pub mod ui;

/// Application crate version string
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

//tcp.port == 42230 || tcp.port == 44325 || tcp.port == 443 || tcp.port == 10853
fn main() {
    // Initialize logging
    env_logger::builder()
        .filter_module("pocket_ark_client", log::LevelFilter::Debug)
        .init();

    // Attempt to apply the hosts file modification guard
    let _host_guard: Option<HostEntryGuard> = HostEntryGuard::apply();

    // Load the config file
    let config: Option<config::ClientConfig> = read_config_file();

    // Load the client identity
    let identity: Option<reqwest::Identity> = load_identity();

    // Create the internal HTTP client
    let client: reqwest::Client =
        create_http_client(identity).expect("Failed to create HTTP client");

    // Initialize the UI
    ui::init(config, client);
}

/// Attempts to load an identity file if one is present
fn load_identity() -> Option<reqwest::Identity> {
    // Load the client identity
    let identity_file = Path::new("pocket-ark-identity.p12");

    // Handle no identity or user declining identity
    if !identity_file.exists() || !show_confirm(
        "Found client identity",
        "Detected client identity pocket-ark-identity.p12, would you like to use this identity?",
    ) {
        return None;
    }

    // Read the client identity
    match read_client_identity(identity_file) {
        Ok(value) => Some(value),
        Err(err) => {
            error!("Failed to set client identity: {}", err);
            show_error("Failed to set client identity", &err.to_string());
            None
        }
    }
}

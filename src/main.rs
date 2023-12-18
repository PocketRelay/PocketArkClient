#![cfg_attr(
    all(target_os = "windows", not(debug_assertions),),
    windows_subsystem = "windows"
)]

use api::{try_create, try_login, try_lookup_host, AuthError, LookupData, LookupError};
use tokio::sync::RwLock;
mod constants;
mod servers;
mod ui;

pub mod api;
pub mod host;
pub mod patch;

/// Shared target location
pub static TARGET: RwLock<Option<LookupData>> = RwLock::const_new(None);
/// Authentication token
pub static TOKEN: RwLock<Option<String>> = RwLock::const_new(None);

//tcp.port == 42230 || tcp.port == 44325 || tcp.port == 443 || tcp.port == 10853
fn main() {
    // Enable tracing
    std::env::set_var("RUST_LOG", "trace");

    // Initialize logging
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .init();

    // Create tokio async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");

    // Add the hosts file entry
    let _ = host::set_host_entry();

    runtime.spawn(servers::start());
    ui::iced::init(runtime);
}

/// Attempts to update the host target first looks up the
/// target then will assign the stored global target to the
/// target before returning the result
///
/// `target` The target to use
async fn try_update_host(target: String) -> Result<LookupData, LookupError> {
    let result = try_lookup_host(target).await?;
    let mut write = TARGET.write().await;
    *write = Some(result.clone());
    Ok(result)
}

async fn try_update_login(username: String, password: String) -> Result<(), AuthError> {
    let token = try_login(username, password).await?;
    let mut write = TOKEN.write().await;
    *write = Some(token);
    Ok(())
}

async fn try_update_create(username: String, password: String) -> Result<(), AuthError> {
    let token = try_create(username, password).await?;
    let mut write = TOKEN.write().await;
    *write = Some(token);
    Ok(())
}

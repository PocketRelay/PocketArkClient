use native_dialog::MessageDialog;

use serde::Deserialize;
use thiserror::Error;
use tokio::sync::RwLock;
mod constants;
mod servers;

//tcp.port == 42230 || tcp.port == 44325 || tcp.port == 443 || tcp.port == 10853
fn main() {
    // Create tokio async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");

    *TARGET.blocking_write() = Some(LookupData {
        host: "localhost".to_string(),
        scheme: "https".to_string(),
        version: "1.0".to_string(),
        port: 443,
    });

    // runtime.spawn(servers::certs::start_server());
    runtime.spawn(servers::redirector::start_server());
    runtime.block_on(servers::main::start_server());
}

/// Shows a native error dialog with the provided title and text
///
/// `title` The title of the dialog
/// `text`  The text of the dialog
pub fn show_error(title: &str, text: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Error)
        .show_alert()
        .unwrap()
}

/// Shared target location
pub static TARGET: RwLock<Option<LookupData>> = RwLock::const_new(None);

/// Details provided by the server. These are the only fields
/// that we need the rest are ignored by this client.
#[derive(Deserialize)]
struct ServerDetails {
    /// The Pocket Relay version of the server
    version: String,
}

/// Data from completing a lookup contains the resolved address
/// from the connection to the server as well as the server
/// version obtained from the server
#[derive(Debug, Clone)]
pub struct LookupData {
    /// The scheme used to connect to the server (e.g http or https)
    scheme: String,
    /// The host address of the server
    host: String,
    /// The server version
    version: String,
    /// The server port
    port: u16,
}

/// Errors that can occur while looking up a server
#[derive(Debug, Error)]
enum LookupError {
    /// The server url was missing the host portion
    #[error("Unable to find host portion of provided Connection URL")]
    InvalidHostTarget,
    /// The server connection failed
    #[error("Failed to connect to server")]
    ConnectionFailed(reqwest::Error),
    /// The server gave an invalid response likely not a PR server
    #[error("Invalid server response")]
    InvalidResponse(reqwest::Error),
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

/// Attempts to connect to the Pocket Relay HTTP server at the provided
/// host. Will make a connection to the /api/server endpoint and if the
/// response is a valid ServerDetails message then the server is
/// considered valid.
///
/// `host` The host to try and lookup
async fn try_lookup_host(host: String) -> Result<LookupData, LookupError> {
    let mut url = String::new();

    // Fill in missing host portion
    if !host.starts_with("http://") && !host.starts_with("https://") {
        url.push_str("http://");
        url.push_str(&host)
    } else {
        url.push_str(&host);
    }

    if !host.ends_with('/') {
        url.push('/')
    }

    url.push_str("ark/client/details");

    let response = reqwest::get(url)
        .await
        .map_err(LookupError::ConnectionFailed)?;

    let url = response.url();
    let scheme = url.scheme().to_string();

    let port = url.port_or_known_default().unwrap_or(80);
    let host = match url.host() {
        Some(value) => value.to_string(),
        None => return Err(LookupError::InvalidHostTarget),
    };

    let details = response
        .json::<ServerDetails>()
        .await
        .map_err(LookupError::InvalidResponse)?;

    Ok(LookupData {
        scheme,
        host,
        port,
        version: details.version,
    })
}

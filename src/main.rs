use std::{
    fs::{copy, read, remove_file, write},
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    string::FromUtf8Error,
};

use constants::{ANSEL_SDK64_BAK, ANSEL_SDK64_DLL, HOSTS_PATH, HOST_KEY, HOST_VALUE};
use native_dialog::{FileDialog, MessageDialog};

use reqwest::Client;
use serde::Deserialize;
use thiserror::Error;
use tokio::sync::RwLock;
mod constants;
mod servers;
mod ui;

//tcp.port == 42230 || tcp.port == 44325 || tcp.port == 443 || tcp.port == 10853
fn main() {
    // Create tokio async runtime
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed building the Runtime");

    // Add the hosts file entry
    let _ = set_host_entry();

    // *TARGET.blocking_write() = Some(LookupData {
    //     host: "localhost".to_string(),
    //     scheme: "https".to_string(),
    //     version: "1.0".to_string(),
    //     port: 443,
    // });

    runtime.spawn(servers::start());
    ui::iced::init(runtime);
}

/// Shows a native info dialog with the provided title and text
///
/// `title` The title of the dialog
/// `text`  The text of the dialog
pub fn show_info(title: &str, text: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Info)
        .show_alert()
        .unwrap()
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

/// Errors that could occur while working with the hosts file
#[derive(Debug, Error)]
enum HostsError {
    /// Hosts file doesn't exist
    #[error("Missing hosts file")]
    FileMissing,
    /// Missing admin permission to access file
    #[error("Missing permission to modify hosts file. Ensure this program is running as admin")]
    PermissionsError,
    /// Failed to read the hosts file
    #[error("Failed to read hosts file: {0}")]
    ReadFailure(io::Error),
    /// Failed to write the hosts file
    #[error("Failed to write hosts file: {0}")]
    WriteFailure(io::Error),
    /// File contained non-utf8 characters
    #[error("Hosts file contained non-utf8 characters so could not be parsed.")]
    NonUtf8(#[from] FromUtf8Error),
}

/// Attempts to read the hosts file contents to a string
/// returning a HostsError if it was unable to do so
fn read_hosts_file() -> Result<String, HostsError> {
    let path = Path::new(HOSTS_PATH);
    if !path.exists() {
        return Err(HostsError::FileMissing);
    }

    // Read the hosts file
    let bytes = match read(path) {
        Ok(value) => value,
        Err(err) => {
            // Handle missing permissions
            return Err(if let ErrorKind::PermissionDenied = err.kind() {
                HostsError::PermissionsError
            } else {
                HostsError::ReadFailure(err)
            });
        }
    };

    // Parse the file contents
    let text = String::from_utf8(bytes)?;
    Ok(text)
}

/// Attempts to write the hosts file contents from a string
/// returning a HostsError if it was unable to do so
fn write_hosts_file(value: &str) -> Result<(), HostsError> {
    let path = Path::new(HOSTS_PATH);

    if let Err(err) = write(path, value) {
        Err(if let ErrorKind::PermissionDenied = err.kind() {
            HostsError::PermissionsError
        } else {
            HostsError::WriteFailure(err)
        })
    } else {
        Ok(())
    }
}

/// Filters lines based on whether or not they are a redirect for
/// the host address. Filters out lines that are commented out
/// / are invalid.
///
/// `value` The line to check
fn filter_not_host_line(value: &&str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.starts_with('#') || !value.contains(HOST_KEY) {
        return true;
    }

    // Split to the content before any comments
    let value = match value.split_once('#') {
        Some((before, _)) => before.trim(),
        None => value,
    };

    // Check we still have content and contain host
    if value.is_empty() || !value.contains(HOST_KEY) {
        return true;
    }

    let mut parts = value.split_whitespace();

    match parts.next() {
        Some(_) => {}
        None => return true,
    }

    match parts.next() {
        Some(value) => !value.eq(HOST_KEY),
        None => true,
    }
}

/// Filters all the host redirects removing any for the
/// gosredirector.ea.com host
fn remove_host_entry() -> Result<(), HostsError> {
    let contents = read_hosts_file()?;
    let lines = contents
        .lines()
        .filter(filter_not_host_line)
        .collect::<Vec<&str>>();
    let output = lines.join("\n");
    write_hosts_file(&output)?;
    Ok(())
}

/// Updates the hosts file with the entry loaded from the server
/// url
///
/// `url` The lookup url for Pocket Relay
fn set_host_entry() -> Result<(), HostsError> {
    let contents = read_hosts_file()?;

    let mut lines = contents
        .lines()
        .filter(filter_not_host_line)
        .collect::<Vec<&str>>();

    let line = format!("{} {}", HOST_VALUE, HOST_KEY);

    lines.push(&line);

    let output = lines.join("\n");
    write_hosts_file(&output)?;

    Ok(())
}

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

    // Create the request
    let response = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .map_err(LookupError::ConnectionFailed)?
        .get(url)
        .send()
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

/// Errors that can occur while patching the game
#[derive(Debug, Error)]
enum PatchError {
    /// The file picker failed to pick a file
    #[error("Failed to get picked file. Make sure this program is running as administrator")]
    PickFileFailed,
    /// The picked path was missing the game exe
    #[error("The path given doesn't contains the MassEffect.exe executable")]
    MissingGame,
    /// Failed to delete the bink232
    #[error("Failed to delete binkw32.dll you will have to manually unpatch your game: {0}")]
    FailedDelete(io::Error),
    /// Fialed to replace the files
    #[error("Failed to replace binkw32.dll with origin binkw23.ddl: {0}")]
    FailedReplaceOriginal(io::Error),
    /// Failed to write the patch files
    #[error("Failed to write patch file dlls (binkw32.dll and binkw32.dll): {0}")]
    FailedWritingPatchFiles(io::Error),
}

/// Attempt to use the system file picker to pick the path to the
/// Mass Effect 3 executable
fn try_pick_game_path() -> Result<Option<PathBuf>, PatchError> {
    FileDialog::new()
        .set_filename("MassEffectAndromeda.exe")
        .add_filter("Mass Effect Andromeda Executable", &["exe"])
        .show_open_single_file()
        .map_err(|_| PatchError::PickFileFailed)
}

/// Attempts to remove the patch from the provided Mass Effect
/// installation by swapping the binkw32 ddl with binkw23 and
/// deleting the old DLL
fn try_remove_patch() -> Result<bool, PatchError> {
    let path = match try_pick_game_path()? {
        Some(value) => value,
        None => return Ok(false),
    };
    if !path.exists() {
        return Err(PatchError::MissingGame);
    }

    let parent = path.parent().ok_or(PatchError::MissingGame)?;

    let ansel_bak = parent.join("AnselSDK64.bak");
    let ansel = parent.join("AnselSDK64.dll");

    if ansel.exists() {
        remove_file(&ansel).map_err(PatchError::FailedDelete)?;
    }

    if ansel_bak.exists() {
        copy(&ansel_bak, &ansel).map_err(PatchError::FailedReplaceOriginal)?;
        let _ = remove_file(&ansel_bak);
    } else {
        write(&ansel, ANSEL_SDK64_BAK).map_err(PatchError::FailedReplaceOriginal)?;
    }

    Ok(true)
}

/// Attempts to patch the Mass Effect installation at the provided game
/// path. Writes the two embedded DLLs to the game directory.
fn try_patch_game() -> Result<bool, PatchError> {
    let path = match try_pick_game_path()? {
        Some(value) => value,
        None => return Ok(false),
    };
    if !path.exists() {
        return Err(PatchError::MissingGame);
    }
    let parent = path.parent().ok_or(PatchError::MissingGame)?;

    let ansel_bak = parent.join("AnselSDK64.bak");
    let ansel = parent.join("AnselSDK64.dll");

    write(ansel_bak, ANSEL_SDK64_BAK).map_err(PatchError::FailedWritingPatchFiles)?;
    write(ansel, ANSEL_SDK64_DLL).map_err(PatchError::FailedWritingPatchFiles)?;
    Ok(true)
}

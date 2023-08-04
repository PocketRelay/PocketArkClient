use std::{
    fs::{read, write},
    io::{self, ErrorKind},
    path::Path,
    string::FromUtf8Error,
};

use thiserror::Error;

use crate::constants::{HOSTS_PATH, HOST_KEY, HOST_VALUE};

/// Filters all the host redirects removing any for the
/// gosredirector.ea.com host
pub fn remove_host_entry() -> Result<(), HostsError> {
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
pub fn set_host_entry() -> Result<(), HostsError> {
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

/// Errors that could occur while working with the hosts file
#[derive(Debug, Error)]
pub enum HostsError {
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

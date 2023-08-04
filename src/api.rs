//! API Logic for working with the Pocket Ark server

use std::ops::Deref;

use hyper::StatusCode;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::TARGET;

/// Details provided by the server. These are the only fields
/// that we need the rest are ignored by this client.
#[derive(Deserialize)]
struct ServerDetails {
    ident: String,
    /// The Pocket Relay version of the server
    version: String,
}

/// Data from completing a lookup contains the resolved address
/// from the connection to the server as well as the server
/// version obtained from the server
#[derive(Debug, Clone)]
pub struct LookupData {
    /// The scheme used to connect to the server (e.g http or https)
    pub scheme: String,
    /// The host address of the server
    pub host: String,
    /// The server version
    pub version: String,
    /// The server port
    pub port: u16,
}

/// Errors that can occur while looking up a server
#[derive(Debug, Error)]
pub enum LookupError {
    /// The server url was missing the host portion
    #[error("Unable to find host portion of provided Connection URL")]
    InvalidHostTarget,
    /// The server connection failed
    #[error("Failed to connect to server")]
    ConnectionFailed(reqwest::Error),
    /// The server gave an invalid response likely not a PA server
    #[error("Invalid server response")]
    InvalidResponse(reqwest::Error),
    /// The server ident was invalid
    #[error("Invalid server ident likely not a Pocket Ark server")]
    InvalidIdent(String),
    /// Server gave non 200 status code
    #[error("Server response gave status code {0}")]
    ErrorResponse(StatusCode),
}

/// Creates an HTTP client setup to work with the Pocket Ark server
pub fn create_http_client() -> Client {
    Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .expect("Failed to create HTTP client")
}

pub fn create_target_url(target: &LookupData, endpoint: &str) -> String {
    let mut url = String::new();
    url.push_str(&target.scheme);
    url.push_str("://");
    url.push_str(&target.host);
    url.push_str(endpoint);
    url
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("Missing target not connected")]
    MissingTarget,
    /// The server connection failed
    #[error("Failed to connect to server")]
    ConnectionFailed(reqwest::Error),
    /// The server gave an invalid response likely not a PA server
    #[error("Invalid server response")]
    InvalidResponse(reqwest::Error),
    #[error("{0}")]
    ErrorResponse(String),
}

#[derive(Debug, Deserialize)]
pub struct HttpError {
    pub reason: String,
}

const LOGIN_ENDPOINT: &str = "/ark/client/login";
const CREATE_ENDPOINT: &str = "/ark/client/create";
const DETAILS_ENDPOINT: &str = "/ark/client/details";

#[derive(Debug, Serialize)]
pub struct AuthRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn try_login(username: String, password: String) -> Result<String, AuthError> {
    let url = {
        let target = &*TARGET.read().await;
        let target = target.as_ref().ok_or(AuthError::MissingTarget)?;
        create_target_url(target, LOGIN_ENDPOINT)
    };

    let response = create_http_client()
        .post(url)
        .json(&AuthRequest { username, password })
        .send()
        .await
        .map_err(AuthError::ConnectionFailed)?;

    let status = response.status();
    if !status.is_success() {
        let err = match response.json::<HttpError>().await {
            Ok(value) => value.reason,
            Err(_) => "Unknown error occurred".to_string(),
        };

        return Err(AuthError::ErrorResponse(err));
    }

    let response: AuthResponse = response.json().await.map_err(AuthError::InvalidResponse)?;
    Ok(response.token)
}

pub async fn try_create(username: String, password: String) -> Result<String, AuthError> {
    let url = {
        let target = &*TARGET.read().await;
        let target = target.as_ref().ok_or(AuthError::MissingTarget)?;
        create_target_url(target, CREATE_ENDPOINT)
    };

    let response = create_http_client()
        .post(url)
        .json(&AuthRequest { username, password })
        .send()
        .await
        .map_err(AuthError::ConnectionFailed)?;

    let status = response.status();
    if !status.is_success() {
        let err = match response.json::<HttpError>().await {
            Ok(value) => value.reason,
            Err(_) => "Unknown error occurred".to_string(),
        };

        return Err(AuthError::ErrorResponse(err));
    }

    let response: AuthResponse = response.json().await.map_err(AuthError::InvalidResponse)?;
    Ok(response.token)
}

/// Attempts to connect to the Pocket Relay HTTP server at the provided
/// host. Will make a connection to the /ark/client/details endpoint and if the
/// response is a valid ServerDetails message then the server is
/// considered valid.
///
/// `host` The host to try and lookup
pub async fn try_lookup_host(host: String) -> Result<LookupData, LookupError> {
    let mut url = String::new();

    // Fill in missing host portion
    if !host.starts_with("http://") && !host.starts_with("https://") {
        url.push_str("http://");
        url.push_str(&host)
    } else {
        url.push_str(&host);
    }

    if url.ends_with('/') {
        let _ = url.pop();
    }

    url.push_str(DETAILS_ENDPOINT);

    // Create the request
    let response = create_http_client()
        .get(url)
        .send()
        .await
        .map_err(LookupError::ConnectionFailed)?;

    let status = response.status();
    if !status.is_success() {
        return Err(LookupError::ErrorResponse(status));
    }

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

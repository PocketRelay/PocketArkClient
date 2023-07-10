use std::{net::Ipv4Addr, process::exit};

use hyper::{header, http::HeaderValue, HeaderMap};
use reqwest::Client;
use tokio::{
    io::copy_bidirectional,
    net::{TcpListener, TcpStream},
};

use crate::{constants::MAIN_PORT, show_error, TARGET};

pub async fn start_server() {
    // Initializing the underlying TCP listener
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, MAIN_PORT)).await {
        Ok(value) => value,
        Err(err) => {
            let text = format!("Failed to start main: {}", err);
            show_error("Failed to start", &text);
            exit(1);
        }
    };

    while let Ok((stream, _addr)) = listener.accept().await {
        println!("Hit main");
        tokio::spawn(handle_client(stream));
    }
}

/// Header for the Pocket Relay connection scheme used by the client
const HEADER_SCHEME: &str = "X-Pocket-Ark-Scheme";
/// Header for the Pocket Relay connection port used by the client
const HEADER_PORT: &str = "X-Pocket-Ark-Port";
/// Header for the Pocket Relay connection host used by the client
const HEADER_HOST: &str = "X-Pocket-Ark-Host";
const HEADER_AUTH: &str = "X-Pocket-Ark-Auth";
/// Endpoint for upgrading the server connection
const UPGRADE_ENDPOINT: &str = "/ark/client/upgrade";

async fn handle_client(mut client: TcpStream) {
    let target = match &*TARGET.read().await {
        Some(value) => value.clone(),
        None => return,
    };

    // Create the upgrade URL
    let mut url = String::new();
    url.push_str(&target.scheme);
    url.push_str("://");
    url.push_str(&target.host);
    url.push_str(UPGRADE_ENDPOINT);

    // Create the HTTP Upgrade headers
    let mut headers = HeaderMap::new();
    headers.insert(header::CONNECTION, HeaderValue::from_static("Upgrade"));
    headers.insert(header::UPGRADE, HeaderValue::from_static("blaze"));

    // Append the schema header
    if let Ok(scheme_value) = HeaderValue::from_str(&target.scheme) {
        headers.insert(HEADER_SCHEME, scheme_value);
    }

    // Append the port header
    headers.insert(HEADER_PORT, HeaderValue::from(target.port));
    headers.insert(HEADER_AUTH, HeaderValue::from_static("wdawdawdawdwdawd"));

    // Append the host header
    if let Ok(host_value) = HeaderValue::from_str(&target.host) {
        headers.insert(HEADER_HOST, host_value);
    }
    println!("connect");

    // Create the request
    let request = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap()
        .get(url)
        .headers(headers)
        .send();

    // Await the server response to the request
    let response = match request.await {
        Ok(value) => value,
        Err(err) => {
            eprintln!("{}", err);
            return;
        }
    };
    println!("Connected");

    // Server connection gained through upgrading the client
    let mut server = match response.upgrade().await {
        Ok(value) => value,
        Err(_) => return,
    };

    println!("Upgraded");

    // Copy the data between the connection
    let _ = copy_bidirectional(&mut client, &mut server).await;
}

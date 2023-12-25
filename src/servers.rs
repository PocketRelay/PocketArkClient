use crate::{
    core::{api::AuthToken, reqwest, servers::*, ssl::create_ssl_context, Url},
    ui::show_error,
};
use log::error;
use std::sync::Arc;

/// Starts all the servers in their own tasks
///
/// ## Arguments
/// * `http_client` - The HTTP client to use on the servers
/// * `base_url`    - The base URL of the connected server
/// * `association` - Optional association token if supported
pub fn start_all_servers(
    http_client: reqwest::Client,
    base_url: Arc<Url>,
    association: Arc<Option<String>>,
    token: AuthToken,
) {
    // Stop existing servers and tasks if they are running
    stop_server_tasks();

    let ssl_context = create_ssl_context().expect("Failed to create ssl context");

    let a = ssl_context.clone();

    // Spawn the Redirector server
    spawn_server_task(async move {
        if let Err(err) = redirector::start_redirector_server(a).await {
            show_error("Failed to start redirector server", &err.to_string());
            error!("Failed to start redirector server: {}", err);
        }
    });

    // Need to copy the client and base_url so it can be moved into the task
    let (a, b, c, d) = (
        http_client.clone(),
        base_url.clone(),
        association.clone(),
        token.clone(),
    );

    // Spawn the Blaze server
    spawn_server_task(async move {
        if let Err(err) = blaze::start_blaze_server(a, b, c, d).await {
            show_error("Failed to start blaze server", &err.to_string());
            error!("Failed to start blaze server: {}", err);
        }
    });

    // Need to copy the client and base_url so it can be moved into the task
    let (a, b) = (http_client.clone(), base_url.clone());

    // Spawn the HTTP server
    spawn_server_task(async move {
        if let Err(err) = http::start_http_server(a, b, ssl_context, token).await {
            show_error("Failed to start http server", &err.to_string());
            error!("Failed to start http server: {}", err);
        }
    });

    // Need to copy the client and base_url so it can be moved into the task
    // let (a, b) = (http_client.clone(), base_url.clone());
    // Spawn the tunneling server (Not supported yet)
    // spawn_server_task(async move {
    //     if let Err(err) = tunnel::start_tunnel_server(a, b, association).await {
    //         show_error("Failed to start tunnel server", &err.to_string());
    //         error!("Failed to start tunnel server: {}", err);
    //     }
    // });

    // Spawn the QoS server
    spawn_server_task(async move {
        if let Err(err) = qos::start_qos_server().await {
            show_error("Failed to start qos server", &err.to_string());
            error!("Failed to start qos server: {}", err);
        }
    });
}

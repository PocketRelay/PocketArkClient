use tokio::join;

// pub mod certs;
pub mod http;
pub mod main;
pub mod qos;
pub mod redirector;

/// Starts and waits for all the servers
pub async fn start() {
    join!(
        main::start_server(),
        qos::start_server(),
        redirector::start_server(),
        http::start_server(),
        // certs::start_server()
    );
}

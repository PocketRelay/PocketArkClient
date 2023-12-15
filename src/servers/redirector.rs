use std::{convert::Infallible, net::Ipv4Addr, pin::Pin, process::exit};

use hyper::{
    header::{HeaderValue, CONTENT_TYPE},
    server::conn::Http,
    service::service_fn,
    Response,
};
use log::{debug, error};
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    ssl::{Ssl, SslAcceptor, SslMethod},
    x509::X509,
};
use tokio::net::TcpListener;

use crate::{
    constants::{MAIN_PORT, REDIRECTOR_PORT},
    ui::show_error,
};

const CERTIFICATE: &[u8] = include_bytes!("../resources/identity/cert.der");
const PRIVATE_KEY: &[u8] = include_bytes!("../resources/identity/key.pem");

pub async fn start_server() {
    let acceptor = {
        let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls_server()).unwrap();

        let crt = X509::from_der(CERTIFICATE).expect("Redirector server certificate is invalid");
        let pkey = PKey::from_rsa(
            Rsa::private_key_from_pem(PRIVATE_KEY)
                .expect("Redirector server private key is invalid"),
        )
        .expect("Server private key is invalid");

        acceptor
            .set_certificate(&crt)
            .expect("Failed to set SSL certificate");
        acceptor
            .set_private_key(&pkey)
            .expect("Failed to set SSL private key");

        acceptor.build()
    };

    // Initializing the underlying TCP listener
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, REDIRECTOR_PORT)).await {
        Ok(value) => value,
        Err(err) => {
            let text = format!("Failed to start http: {}", err);
            show_error("Failed to start", &text);
            exit(1);
        }
    };

    // Accept incoming connections
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(value) => value,
            Err(_) => break,
        };

        let ssl = Ssl::new(acceptor.context()).unwrap();

        tokio::task::spawn(async move {
            debug!("redirect hit");

            let mut stream = match tokio_openssl::SslStream::new(ssl, stream) {
                Ok(value) => value,
                Err(err) => {
                    error!("Failed to accept ssl connection: {}", err);
                    return;
                }
            };

            Pin::new(&mut stream).accept().await.unwrap();

            if let Err(err) = Http::new()
                .serve_connection(stream, service_fn(handle_http))
                .await
            {
                error!("Failed to serve http connection: {:?}", err);
            }
        });
    }
}

const BLAZE_COMPONENT: &str = "X-BLAZE-COMPONENT";
const BLAZE_COMMAND: &str = "X-BLAZE-COMMAND";
const BLAZE_SEQ: &str = "X-BLAZE-SEQNO";

async fn handle_http(
    _req: hyper::Request<hyper::body::Body>,
) -> Result<hyper::Response<hyper::body::Body>, Infallible> {
    let addr = u32::from_be_bytes([127, 0, 0, 1]);
    let res = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
    <serverinstanceinfo>
        <address member="0">
            <valu>
                <hostname>localhost</hostname>
                <ip>{}</ip>
                <port>{}</port>
            </valu>
        </address>
        <secure>0</secure>
        <trialservicename></trialservicename>
        <defaultdnsaddress>0</defaultdnsaddress>
    </serverinstanceinfo>"#,
        addr, MAIN_PORT
    );

    Ok(Response::builder()
        .header(BLAZE_COMPONENT, HeaderValue::from_static("redirector"))
        .header(BLAZE_COMMAND, HeaderValue::from_static("getServerInstance"))
        .header(BLAZE_SEQ, HeaderValue::from_static("0"))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/xml"))
        .body(res.into())
        .unwrap())
}

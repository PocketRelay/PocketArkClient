use std::{
    net::{Ipv4Addr, SocketAddr, SocketAddrV4},
    process::exit,
};

use axum::{
    response::{IntoResponse, Response},
    routing::any,
    Router,
};
use axum_server::tls_openssl::OpenSSLConfig;
use hyper::{header, http::HeaderValue};
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    ssl::{SslAcceptor, SslMethod},
    x509::X509,
};
use tower_http::trace::TraceLayer;

use crate::{
    constants::{MAIN_PORT, REDIRECTOR_PORT},
    show_error,
};

const CERTIFICATE: &[u8] = include_bytes!("../resources/identity/cert.der");
const PRIVATE_KEY: &[u8] = include_bytes!("../resources/identity/key.pem");

pub async fn start_server() {
    let addr: SocketAddr =
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, REDIRECTOR_PORT));

    let router = Router::new()
        .route("/redirector/getServerInstance", any(handle_get_instance))
        .layer(TraceLayer::new_for_http());
    let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls_server()).unwrap();

    let crt = X509::from_der(CERTIFICATE).expect("Redirector server certificate is invalid");
    let pkey = PKey::from_rsa(
        Rsa::private_key_from_pem(PRIVATE_KEY).expect("Redirector server private key is invalid"),
    )
    .expect("Server private key is invalid");

    acceptor
        .set_certificate(&crt)
        .expect("Failed to set redirector server certificate");
    acceptor
        .set_private_key(&pkey)
        .expect("Failed to set redirector server private key");

    let config = OpenSSLConfig::try_from(acceptor).expect("Failed to create OpenSSL config");

    if let Err(err) = axum_server::bind_openssl(addr, config)
        .serve(router.into_make_service())
        .await
    {
        show_error("Failed to start redirector server", &err.to_string());
        exit(1);
    }
}

async fn handle_get_instance() -> Response {
    println!("Hit redirector");
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
    let mut res: Response = res.into_response();
    res.headers_mut()
        .insert("X-BLAZE-COMPONENT", HeaderValue::from_static("redirector"));
    res.headers_mut().insert(
        "X-BLAZE-COMMAND",
        HeaderValue::from_static("getServerInstance"),
    );
    res.headers_mut()
        .insert("X-BLAZE-SEQNO", HeaderValue::from_static("0"));
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml"),
    );

    res
}

use crate::{api::create_target_url, constants::HTTPS_PORT, ui::show_error, TARGET, TOKEN};
use hyper::{
    header::{HeaderValue, CONTENT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE},
    server::conn::Http,
    service::service_fn,
    HeaderMap, Method, Response, StatusCode,
};
use log::debug;
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    ssl::{Ssl, SslAcceptor, SslMethod},
    x509::X509,
};
use std::{convert::Infallible, net::Ipv4Addr, pin::Pin, process::exit};
use tokio::net::TcpListener;

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
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, HTTPS_PORT)).await {
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
            let mut stream = match tokio_openssl::SslStream::new(ssl, stream) {
                Ok(value) => value,
                Err(err) => {
                    eprintln!("Failed to accept ssl connection: {}", err);
                    return;
                }
            };

            Pin::new(&mut stream).accept().await.unwrap();

            if let Err(err) = Http::new()
                .serve_connection(stream, service_fn(handle_http))
                .await
            {
                eprintln!("Failed to serve http connection: {:?}", err);
            }
        });
    }
}

const TOKEN_HEADER: &str = "X-Token";

async fn handle_http(
    req: hyper::Request<hyper::body::Body>,
) -> Result<hyper::Response<hyper::body::Body>, Infallible> {
    debug!("{:?}", req);
    // TODO: Security, handle non local connections prevent them from using this token

    let target = match &*TARGET.read().await {
        Some(value) => value.clone(),
        None => {
            // Target not available
            return Ok(Response::builder()
                .status(StatusCode::SERVICE_UNAVAILABLE)
                .body(hyper::Body::empty())
                .unwrap());
        }
    };
    let req_headers = req.headers();

    let uri = req.uri();
    let path = uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or_default();

    // Create the upgrade URL
    let url = create_target_url(&target, path);

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let mut proxy_req = client.request(req.method().clone(), url);

    // Provide token authentication if present
    if let Some(token) = &*TOKEN.read().await {
        proxy_req = proxy_req.header(TOKEN_HEADER, HeaderValue::from_str(token).unwrap());
    }

    if req.method() == Method::POST
        || req.method() == Method::PUT
        || req.method() == Method::DELETE
        || req.method() == Method::PATCH
    {
        if let Some(length) = req_headers.get(CONTENT_LENGTH) {
            proxy_req = proxy_req.header(CONTENT_LENGTH, length.clone());
        }

        if let Some(content_type) = req_headers.get(CONTENT_TYPE) {
            proxy_req = proxy_req.header(CONTENT_TYPE, content_type.clone());
        }

        // Forward encoding type
        if let Some(content_type) = req_headers.get(CONTENT_ENCODING) {
            proxy_req = proxy_req.header(CONTENT_ENCODING, content_type.clone());
        }

        proxy_req = proxy_req.body(req.into_body());
    }

    let proxy_res = proxy_req.send().await.unwrap();
    let proxy_res_headers = proxy_res.headers();

    let mut headers_out = HeaderMap::new();

    if let Some(content_type) = proxy_res_headers.get(CONTENT_TYPE) {
        headers_out.insert(CONTENT_TYPE, content_type.clone());
    }

    let status = proxy_res.status();

    let body = proxy_res.bytes().await.unwrap();

    let mut response = Response::new(hyper::body::Body::from(body));
    *response.status_mut() = status;
    *response.headers_mut() = headers_out;

    Ok(response)
}

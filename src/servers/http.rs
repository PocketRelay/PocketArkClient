use std::{convert::Infallible, net::Ipv4Addr, pin::Pin, process::exit};

use hyper::{
    header::{HeaderValue, CONTENT_LENGTH, CONTENT_TYPE},
    server::conn::Http,
    service::service_fn,
    HeaderMap, Method, Response, StatusCode,
};
use openssl::{
    pkey::PKey,
    rsa::Rsa,
    ssl::{Ssl, SslAcceptor, SslMethod},
    x509::X509,
};
use tokio::net::TcpListener;

use crate::{
    api::create_target_url,
    constants::{HTTP_PORT, MAIN_PORT},
    ui::show_error,
    TARGET, TOKEN,
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
    let listener = match TcpListener::bind((Ipv4Addr::UNSPECIFIED, HTTP_PORT)).await {
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

const BLAZE_COMPONENT: &str = "X-BLAZE-COMPONENT";
const BLAZE_COMMAND: &str = "X-BLAZE-COMMAND";
const BLAZE_SEQ: &str = "X-BLAZE-SEQNO";
const TOKEN_HEADER: &str = "X-Token";

async fn handle_http(
    req: hyper::Request<hyper::body::Body>,
) -> Result<hyper::Response<hyper::body::Body>, Infallible> {
    // TODO: Security, handle non local connections prevent them from using this token

    dbg!(&req);

    let uri = req.uri();

    // Handle redirect requests locally
    if uri.path().eq("/redirector/getServerInstance") {
        return Ok(server_instance_response());
    }

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

fn server_instance_response() -> hyper::Response<hyper::body::Body> {
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

    Response::builder()
        .header(BLAZE_COMPONENT, HeaderValue::from_static("redirector"))
        .header(BLAZE_COMMAND, HeaderValue::from_static("getServerInstance"))
        .header(BLAZE_SEQ, HeaderValue::from_static("0"))
        .header(CONTENT_TYPE, HeaderValue::from_static("application/xml"))
        .body(res.into())
        .unwrap()
}

// pub async fn start_server() {
//     let addr: SocketAddr =
//         SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, REDIRECTOR_PORT));

//     let router = Router::new()
//         .route("/redirector/getServerInstance", any(handle_get_instance))
//         .fallback(service_fn(route_fallback))
//         .layer(TraceLayer::new_for_http());
//     let mut acceptor = SslAcceptor::mozilla_intermediate(SslMethod::tls_server()).unwrap();

//     let crt = X509::from_der(CERTIFICATE).expect("Redirector server certificate is invalid");
// let pkey = PKey::from_rsa(
//     Rsa::private_key_from_pem(PRIVATE_KEY).expect("Redirector server private key is invalid"),
// )
// .expect("Server private key is invalid");

//     acceptor
//         .set_certificate(&crt)
//         .expect("Failed to set redirector server certificate");
//     acceptor
//         .set_private_key(&pkey)
//         .expect("Failed to set redirector server private key");

//     let config = OpenSSLConfig::try_from(acceptor).expect("Failed to create OpenSSL config");

//     if let Err(err) = axum_server::bind_openssl(addr, config)
//         .serve(router.into_make_service())
//         .await
//     {
//         show_error("Failed to start redirector server", &err.to_string());
//         exit(1);
//     }
// }

// async fn handle_get_instance() -> Response {
//     println!("Hit redirector");
//     let addr = u32::from_be_bytes([127, 0, 0, 1]);
//     let res = format!(
//         r#"<?xml version="1.0" encoding="UTF-8"?>
//     <serverinstanceinfo>
//         <address member="0">
//             <valu>
//                 <hostname>localhost</hostname>
//                 <ip>{}</ip>
//                 <port>{}</port>
//             </valu>
//         </address>
//         <secure>0</secure>
//         <trialservicename></trialservicename>
//         <defaultdnsaddress>0</defaultdnsaddress>
//     </serverinstanceinfo>"#,
//         addr, MAIN_PORT
//     );
//     let mut res: Response = res.into_response();
//     res.headers_mut()
//         .insert("X-BLAZE-COMPONENT", HeaderValue::from_static("redirector"));
//     res.headers_mut().insert(
//         "X-BLAZE-COMMAND",
//         HeaderValue::from_static("getServerInstance"),
//     );
//     res.headers_mut()
//         .insert("X-BLAZE-SEQNO", HeaderValue::from_static("0"));
//     res.headers_mut().insert(
//         header::CONTENT_TYPE,
//         HeaderValue::from_static("application/xml"),
//     );

//     res
// }

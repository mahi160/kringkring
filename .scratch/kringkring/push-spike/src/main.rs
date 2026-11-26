mod push;
mod tls;

use std::{
    error::Error,
    net::{Ipv4Addr, SocketAddr},
};

use axum::{
    Router,
    response::{Html, IntoResponse},
    routing::{get, post},
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [ip, contact] = args.as_slice() else {
        return Err("usage: cargo run -- <LAN IPv4> <VAPID contact email>".into());
    };
    let ip: Ipv4Addr = ip.parse()?;
    if ip.is_unspecified() || !contact.contains('@') {
        return Err("provide a concrete LAN IPv4 address and a VAPID contact email".into());
    }
    let http_addr = SocketAddr::from((ip, 8080));
    let https_addr = SocketAddr::from((ip, 8443));
    let origin = format!("https://{https_addr}");
    let (ca, tls) = tls::generate(ip).await?;
    let push = push::Push::new(origin.clone(), format!("mailto:{contact}"))?;

    let http = Router::new().route(
        "/ca.cer",
        get(move || async move {
            (
                [
                    ("content-type", "application/x-x509-ca-cert"),
                    (
                        "content-disposition",
                        "attachment; filename=kringkring-spike-ca.cer",
                    ),
                ],
                ca,
            )
        }),
    );
    let https = Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../web/index.html")) }),
        )
        .route(
            "/manifest.webmanifest",
            get(|| async {
                asset(
                    "application/manifest+json",
                    include_str!("../web/manifest.webmanifest"),
                )
            }),
        )
        .route(
            "/icon.svg",
            get(|| async { asset("image/svg+xml", include_str!("../web/icon.svg")) }),
        )
        .route(
            "/icon-192.png",
            get(|| async {
                asset(
                    "image/png",
                    include_bytes!("../web/icon-192.png").as_slice(),
                )
            }),
        )
        .route(
            "/icon-512.png",
            get(|| async {
                asset(
                    "image/png",
                    include_bytes!("../web/icon-512.png").as_slice(),
                )
            }),
        )
        .route(
            "/app.js",
            get(|| async { asset("text/javascript", include_str!("../web/app.js")) }),
        )
        .route(
            "/sw.js",
            get(|| async { asset("text/javascript", include_str!("../web/sw.js")) }),
        )
        .route("/vapid-public-key", get(push::public_key))
        .route("/subscriptions", post(push::subscribe))
        .route("/send", post(push::send))
        .with_state(push);

    let http_listener = TcpListener::bind(http_addr).await?;
    let https_listener = std::net::TcpListener::bind(https_addr)?;
    https_listener.set_nonblocking(true)?;
    let https_server = axum_server::from_tcp_rustls(https_listener, tls)?;
    println!("CA download: http://{http_addr}/ca.cer");
    println!("Push spike:  {origin}/");
    tokio::try_join!(
        axum::serve(http_listener, http).into_future(),
        https_server.serve(https.into_make_service())
    )?;
    Ok(())
}

fn asset(content_type: &'static str, body: impl IntoResponse) -> impl IntoResponse {
    (
        [
            ("content-type", content_type),
            ("cache-control", "no-store"),
        ],
        body,
    )
}

//! warframe.market client: REST and WebSocket.

pub mod client;
pub mod events;
pub mod rest;

pub(crate) const USER_AGENT: &str = concat!(
    "NasuaFrame/",
    env!("CARGO_PKG_VERSION"),
    " (+https://github.com/GGuedesA/NasuaFrame)"
);

pub(crate) fn install_tls() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

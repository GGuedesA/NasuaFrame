//! WebSocket client for the warframe.market new-orders stream

use std::fmt;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use nasua_core::model::{Order, Platform};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::{self, Message, client::IntoClientRequest, http::HeaderValue};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::USER_AGENT;
use crate::events::parse_new_order;

const WS_URL: &str = "wss://ws.warframe.market/socket";
const SUBSCRIBE_ROUTE: &str = "@wfm|cmd/subscribe/newOrders";
const REPLY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub enum Error {
    /// connection failed or dropped, needs a reconnect
    Socket(tungstenite::Error),
    /// subscribe refused or never answered
    Subscribe(String),
    /// one bad message, skip it and keep reading
    Parse(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Socket(e) => write!(f, "websocket: {e}"),
            Error::Subscribe(reason) => write!(f, "subscribe failed: {reason}"),
            Error::Parse(e) => write!(f, "unreadable message: {e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<tungstenite::Error> for Error {
    fn from(e: tungstenite::Error) -> Self {
        Error::Socket(e)
    }
}

/// subscribed connection, read it with `next_order`
pub struct OrderStream {
    ws: WebSocketStream<MaybeTlsStream<TcpStream>>,
}

/// connects and subscribes to new orders for one platform
pub async fn connect(platform: Platform, crossplay: bool) -> Result<OrderStream, Error> {
    crate::install_tls();
    connect_to(WS_URL, platform, crossplay).await
}

async fn connect_to(url: &str, platform: Platform, crossplay: bool) -> Result<OrderStream, Error> {
    let mut request = url.into_client_request()?;
    let headers = request.headers_mut();
    headers.insert("Sec-WebSocket-Protocol", HeaderValue::from_static("wfm"));
    headers.insert("User-Agent", HeaderValue::from_static(USER_AGENT));
    let (mut ws, _response) = tokio_tungstenite::connect_async(request).await?;

    let subscribe = json!({
        "route": SUBSCRIBE_ROUTE,
        "id": "nasuaframe-new-orders",
        "payload": { "platform": platform_slug(platform), "crossplay": crossplay },
    });
    ws.send(Message::text(subscribe.to_string())).await?;

    match tokio::time::timeout(REPLY_TIMEOUT, wait_for_reply(&mut ws)).await {
        Ok(reply) => reply?,
        Err(_elapsed) => return Err(Error::Subscribe("no reply from the server".into())),
    }
    Ok(OrderStream { ws })
}

/// waits for the subscribe ok/error, skips anything sent before it
async fn wait_for_reply(ws: &mut WebSocketStream<MaybeTlsStream<TcpStream>>) -> Result<(), Error> {
    while let Some(message) = ws.next().await {
        let Message::Text(text) = message? else {
            continue;
        };
        let reply: Value = serde_json::from_str(text.as_str()).unwrap_or_default();
        match reply["route"].as_str() {
            Some(route) if route == format!("{SUBSCRIBE_ROUTE}:ok") => return Ok(()),
            Some(route) if route == format!("{SUBSCRIBE_ROUTE}:error") => {
                return Err(Error::Subscribe(text.to_string()));
            }
            _ => {}
        }
    }
    Err(Error::Socket(tungstenite::Error::ConnectionClosed))
}

impl OrderStream {
    /// next new order, None once the server closes the connection
    pub async fn next_order(&mut self) -> Option<Result<Order, Error>> {
        loop {
            // tungstenite answers pings by itself
            let text = match self.ws.next().await? {
                Ok(Message::Text(text)) => text,
                Ok(Message::Close(_)) => return None,
                Ok(_) => continue,
                Err(e) => return Some(Err(e.into())),
            };
            match parse_new_order(text.as_str()) {
                Ok(Some(order)) => return Some(Ok(order)),
                Ok(None) => continue,
                Err(e) => return Some(Err(Error::Parse(e))),
            }
        }
    }
}

fn platform_slug(platform: Platform) -> &'static str {
    match platform {
        Platform::Pc => "pc",
        Platform::Ps4 => "ps4",
        Platform::Xbox => "xbox",
        Platform::Switch => "switch",
        Platform::Mobile => "mobile",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio::task::JoinHandle;
    use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};

    const OK: &str = r#"{"route":"@wfm|cmd/subscribe/newOrders:ok","id":"nasuaframe-new-orders"}"#;
    const REPORT: &str =
        r#"{"route":"@wfm|event/reports/online","payload":{"connections":1,"authorizedUsers":0}}"#;
    const ORDER: &str = r#"{"route":"@wfm|event/subscriptions/newOrder","payload":{"id":"000000000000000000000001","type":"sell","platinum":75,"itemId":"item","user":{"ingameName":"TestTrader","platform":"pc","crossplay":true,"status":"ingame"}}}"#;
    const BAD_ORDER: &str =
        r#"{"route":"@wfm|event/subscriptions/newOrder","payload":{"type":"sell"}}"#;

    /// local fake server, sends `replies` after the subscribe and then closes
    async fn fake_server(replies: Vec<&'static str>) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            // client refuses a server that doesn't send wfm back
            #[allow(clippy::result_large_err)]
            let accept_wfm =
                |_: &Request, mut response: Response| -> Result<Response, ErrorResponse> {
                    response
                        .headers_mut()
                        .insert("Sec-WebSocket-Protocol", HeaderValue::from_static("wfm"));
                    Ok(response)
                };
            let mut ws = tokio_tungstenite::accept_hdr_async(tcp, accept_wfm)
                .await
                .unwrap();
            let subscribe = ws.next().await.unwrap().unwrap().into_text().unwrap();
            for reply in replies {
                ws.send(Message::text(reply)).await.unwrap();
            }
            ws.close(None).await.unwrap();
            subscribe.to_string()
        });
        (url, server)
    }

    #[tokio::test]
    async fn subscribes_then_yields_orders_and_skips_the_rest() {
        let (url, server) = fake_server(vec![REPORT, OK, REPORT, BAD_ORDER, ORDER]).await;
        let mut stream = connect_to(&url, Platform::Pc, true).await.unwrap();

        assert!(matches!(
            stream.next_order().await,
            Some(Err(Error::Parse(_)))
        ));
        let order = stream.next_order().await.unwrap().unwrap();
        assert_eq!(order.platinum, 75);
        assert!(stream.next_order().await.is_none());

        let subscribe: Value = serde_json::from_str(&server.await.unwrap()).unwrap();
        assert_eq!(subscribe["route"], SUBSCRIBE_ROUTE);
        assert_eq!(subscribe["payload"]["platform"], "pc");
        assert_eq!(subscribe["payload"]["crossplay"], true);
    }

    #[tokio::test]
    async fn refused_subscription_is_an_error() {
        let refused = r#"{"route":"@wfm|cmd/subscribe/newOrders:error","payload":"nope"}"#;
        let (url, _server) = fake_server(vec![refused]).await;
        let result = connect_to(&url, Platform::Pc, true).await;
        assert!(matches!(result, Err(Error::Subscribe(_))));
    }

    #[tokio::test]
    async fn closing_before_the_reply_is_an_error() {
        let (url, _server) = fake_server(vec![REPORT]).await;
        let result = connect_to(&url, Platform::Pc, true).await;
        assert!(matches!(result, Err(Error::Socket(_))));
    }
}

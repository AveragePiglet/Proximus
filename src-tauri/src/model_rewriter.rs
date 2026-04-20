use bytes::Bytes;
use http_body_util::{BodyExt, Full, Either};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;

/// Response body type: either a streamed upstream response or a local error body
type RespBody = Either<Incoming, Full<Bytes>>;

/// Rewrite model names to copilot-api expected IDs
fn rewrite_model(name: &str) -> &str {
    let lower = name.to_lowercase();
    if lower.contains("opus") {
        "claude-opus-4.6"
    } else if lower.contains("haiku") {
        "claude-haiku-4.5"
    } else if lower.contains("sonnet") {
        "claude-sonnet-4.6"
    } else {
        name
    }
}

struct ProxyState {
    upstream: String,
}

fn error_response(status: u16, msg: String) -> Response<RespBody> {
    Response::builder()
        .status(status)
        .body(Either::Right(Full::new(Bytes::from(msg))))
        .unwrap()
}

async fn handle_request(
    state: Arc<ProxyState>,
    req: Request<Incoming>,
) -> Result<Response<RespBody>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();

    // Read the request body (requests are small — not streamed)
    let body_bytes = req.collect().await?.to_bytes();

    // Rewrite model field if JSON
    let final_body = if headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false)
    {
        if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            if let Some(model) = json.get("model").and_then(|m| m.as_str()).map(String::from) {
                let rewritten = rewrite_model(&model);
                if rewritten != model {
                    eprintln!("[rewrite] {} → {}", model, rewritten);
                    json["model"] = serde_json::Value::String(rewritten.to_string());
                }
            }
            serde_json::to_vec(&json).unwrap_or_else(|_| body_bytes.to_vec())
        } else {
            body_bytes.to_vec()
        }
    } else {
        body_bytes.to_vec()
    };

    // Parse upstream address
    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");
    let upstream_url = format!("{}{}", state.upstream, path);
    let upstream_uri: hyper::Uri = match upstream_url.parse() {
        Ok(u) => u,
        Err(e) => return Ok(error_response(502, format!("Bad URI: {}", e))),
    };

    let host = upstream_uri.host().unwrap_or("localhost").to_string();
    let port = upstream_uri.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    // Connect to upstream
    let stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[rewrite] upstream connect error: {}", e);
            return Ok(error_response(502, format!("Bad gateway: {}", e)));
        }
    };

    let io = TokioIo::new(stream);
    let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rewrite] handshake error: {}", e);
            return Ok(error_response(502, format!("Handshake error: {}", e)));
        }
    };

    // Drive the connection in the background
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("[rewrite] connection error: {}", e);
        }
    });

    // Build the forwarded request
    let mut builder = Request::builder().method(method).uri(path);
    for (key, value) in headers.iter() {
        if key != "host" && key != "content-length" {
            builder = builder.header(key, value);
        }
    }
    builder = builder.header("host", format!("{}:{}", host, port));
    builder = builder.header("content-length", final_body.len().to_string());

    let upstream_req = builder
        .body(Full::new(Bytes::from(final_body)))
        .unwrap();

    // Send request and get response — stream it through without buffering
    let upstream_resp = match sender.send_request(upstream_req).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rewrite] upstream request error: {}", e);
            return Ok(error_response(502, format!("Bad gateway: {}", e)));
        }
    };

    // Pass through status + headers + streaming body
    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();

    let mut response = Response::builder().status(status);
    for (key, value) in resp_headers.iter() {
        response = response.header(key, value);
    }

    Ok(response.body(Either::Left(upstream_resp.into_body())).unwrap())
}

/// Start the model-rewrite proxy on `listen_port`, forwarding to `upstream_port`.
/// Spawns via tauri's async runtime and returns immediately.
pub fn start(upstream_port: u16, listen_port: u16) {
    let state = Arc::new(ProxyState {
        upstream: format!("http://localhost:{}", upstream_port),
    });

    tauri::async_runtime::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], listen_port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[rewrite] Failed to bind port {}: {}", listen_port, e);
                return;
            }
        };

        eprintln!(
            "[rewrite] Model-rewrite proxy listening on :{} → http://localhost:{}",
            listen_port, upstream_port
        );

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[rewrite] accept error: {}", e);
                    continue;
                }
            };

            let state = state.clone();
            tokio::spawn(async move {
                let io = TokioIo::new(stream);
                if let Err(e) = http1::Builder::new()
                    .serve_connection(
                        io,
                        service_fn(move |req| {
                            let state = state.clone();
                            handle_request(state, req)
                        }),
                    )
                    .await
                {
                    eprintln!("[rewrite] serve error: {}", e);
                }
            });
        }
    });
}

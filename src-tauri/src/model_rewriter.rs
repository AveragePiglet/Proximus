use bytes::Bytes;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use futures_util::TryStreamExt;

/// Unified response body type — boxes everything so we can return
/// either an upstream hyper stream, a local Full<Bytes>, or a
/// streamed reqwest response without needing a three-way enum.
type RespBody = http_body_util::combinators::BoxBody<
    Bytes,
    Box<dyn std::error::Error + Send + Sync + 'static>,
>;

fn full_body(bytes: impl Into<Bytes>) -> RespBody {
    Full::new(bytes.into())
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
        .boxed()
}

fn incoming_body(incoming: Incoming) -> RespBody {
    incoming
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
        .boxed()
}

/// Rewrite model names from Claude format (dashes) to Copilot format (dots)
/// e.g. "claude-opus-4-6" → "claude-opus-4.6"
/// Also handles proxy aliases: "claude-gpt-5-4" → "gpt-5.4" etc.
fn rewrite_model(name: &str) -> String {
    let lower = name.to_lowercase();

    let aliases: &[(&str, &str)] = &[
        ("claude-gpt-5-4-codex",    "gpt-5.4-codex"),
        ("claude-gpt-5-4-mini",     "gpt-5.4-mini"),
        ("claude-gpt-5-4-nano",     "gpt-5.4-nano"),
        ("claude-gpt-5-4",          "gpt-5.4"),
        ("claude-gpt-5-mini",       "gpt-5-mini"),
        ("claude-gpt-5",            "gpt-5"),
        ("claude-gpt-4-1",          "gpt-4.1"),
        ("claude-gpt-4o-mini",      "gpt-4o-mini"),
        ("claude-gpt-4o",           "gpt-4o"),
        ("claude-gemini-2-5-pro",   "gemini-2.5-pro"),
    ];
    for (alias, target) in aliases {
        if lower == *alias {
            eprintln!("[rewrite] alias {} → {}", name, target);
            return target.to_string();
        }
    }

    let families = ["opus", "sonnet", "haiku"];
    for family in families {
        // Only match claude-<family>-... to avoid accidentally rewriting unrelated
        // model IDs that happen to contain the family name as a substring.
        let prefix = format!("claude-{}-", family);
        if let Some(rest) = lower.strip_prefix(&prefix) {
            if let Some(dash_pos) = rest.rfind('-') {
                let version = format!("{}.{}", &rest[..dash_pos], &rest[dash_pos + 1..]);
                return format!("claude-{}-{}", family, version);
            }
            return format!("claude-{}-{}", family, rest);
        }
    }
    name.to_string()
}

/// Apply model rewrite and field fixups to a JSON body.
/// When `rewrite_model_names` is false (OpenRouter path), the model field is
/// left untouched — provider-qualified IDs like "anthropic/claude-sonnet-4-5"
/// must pass through verbatim.
fn rewrite_json_body(body_bytes: &[u8], rewrite_model_names: bool) -> Vec<u8> {
    if let Ok(mut json) = serde_json::from_slice::<serde_json::Value>(body_bytes) {
        if let Some(model) = json.get("model").and_then(|m| m.as_str()).map(String::from) {
            let rewritten = if rewrite_model_names {
                rewrite_model(&model)
            } else {
                model.clone()
            };
            if rewritten != model {
                eprintln!("[rewrite] {} → {}", model, rewritten);
                json["model"] = serde_json::Value::String(rewritten.clone());
            }
            if !rewritten.starts_with("claude-") {
                if let Some(max_tokens) = json.get("max_tokens").cloned() {
                    json.as_object_mut().map(|o| {
                        o.remove("max_tokens");
                        o.insert("max_completion_tokens".to_string(), max_tokens);
                    });
                    eprintln!("[rewrite] max_tokens → max_completion_tokens for non-Claude model");
                }
            }
        }
        return serde_json::to_vec(&json).unwrap_or_else(|_| body_bytes.to_vec());
    }
    body_bytes.to_vec()
}

struct ProxyState {
    upstream: String,
    openrouter_api_key: Arc<tokio::sync::RwLock<String>>,
}

fn error_response(status: u16, msg: &str) -> Response<RespBody> {
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(full_body(Bytes::from(msg.to_string())))
        .unwrap()
}

async fn handle_request(
    state: Arc<ProxyState>,
    req: Request<Incoming>,
) -> Result<Response<RespBody>, hyper::Error> {
    let method = req.method().clone();
    let uri = req.uri().clone();
    let headers = req.headers().clone();
    let body_bytes = req.collect().await?.to_bytes();

    let is_json = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("application/json"))
        .unwrap_or(false);

    let path = uri.path_and_query().map(|p| p.as_str()).unwrap_or("/");

    let or_key = state.openrouter_api_key.read().await.clone();

    // Only rewrite model names when routing to Copilot (not OpenRouter).
    // OpenRouter expects provider-qualified IDs like "anthropic/claude-sonnet-4-5" verbatim.
    let rewrite_model_names = is_json && or_key.is_empty();
    let final_body = if is_json {
        rewrite_json_body(&body_bytes, rewrite_model_names)
    } else {
        body_bytes.to_vec()
    };

    if !or_key.is_empty() {
        return forward_to_openrouter(&or_key, path, &headers, final_body).await;
    }

    // Copilot-api TCP path
    let upstream_url = format!("{}{}", state.upstream, path);
    let upstream_uri: hyper::Uri = match upstream_url.parse() {
        Ok(u) => u,
        Err(e) => return Ok(error_response(502, &format!("Bad URI: {}", e))),
    };

    let host = upstream_uri.host().unwrap_or("localhost").to_string();
    let port = upstream_uri.port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);

    let stream = match tokio::net::TcpStream::connect(&addr).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[rewrite] upstream connect error: {}", e);
            return Ok(error_response(502, &format!("Bad gateway: {}", e)));
        }
    };

    let io = TokioIo::new(stream);
    let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rewrite] handshake error: {}", e);
            return Ok(error_response(502, &format!("Handshake error: {}", e)));
        }
    };

    tokio::spawn(async move {
        if let Err(e) = conn.await {
            eprintln!("[rewrite] connection error: {}", e);
        }
    });

    let mut builder = Request::builder().method(method).uri(path);
    for (key, value) in headers.iter() {
        if key != "host" && key != "content-length" {
            builder = builder.header(key, value);
        }
    }
    builder = builder.header("host", format!("{}:{}", host, port));
    builder = builder.header("content-length", final_body.len().to_string());

    let upstream_req = builder.body(Full::new(Bytes::from(final_body))).unwrap();

    let upstream_resp = match sender.send_request(upstream_req).await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rewrite] upstream request error: {}", e);
            return Ok(error_response(502, &format!("Bad gateway: {}", e)));
        }
    };

    let status = upstream_resp.status();
    let resp_headers = upstream_resp.headers().clone();
    let mut response = Response::builder().status(status);
    for (key, value) in resp_headers.iter() {
        response = response.header(key, value);
    }
    Ok(response.body(incoming_body(upstream_resp.into_body())).unwrap())
}

/// Forward to OpenRouter via reqwest (HTTPS), streaming the response body
/// back to Claude Code as it arrives — no buffering.
/// Claude Code sends paths like /v1/messages — strip the /v1 prefix since
/// OpenRouter's base URL already includes /api/v1.
async fn forward_to_openrouter(
    api_key: &str,
    path: &str,
    headers: &hyper::HeaderMap,
    body: Vec<u8>,
) -> Result<Response<RespBody>, hyper::Error> {
    let stripped = path.strip_prefix("/v1").unwrap_or(path);
    let url = format!("https://openrouter.ai/api/v1{}", stripped);
    eprintln!("[rewrite] OpenRouter → {}", url);

    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://proximus.app")
        .header("X-Title", "Proximus Workspace")
        .body(body);

    // Forward Anthropic-specific headers so OpenRouter responds in Anthropic format
    for name in &[
        "accept",
        "anthropic-version",
        "anthropic-beta",
        "x-stainless-arch",
        "x-stainless-lang",
        "x-stainless-os",
        "x-stainless-package-version",
        "x-stainless-runtime",
    ] {
        if let Some(val) = headers.get(*name).and_then(|v| v.to_str().ok()) {
            req = req.header(*name, val);
        }
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[rewrite] OpenRouter request error: {}", e);
            return Ok(error_response(502, &format!(
                r#"{{"type":"error","error":{{"type":"api_error","message":"OpenRouter error: {}"}}}}"#,
                e
            )));
        }
    };

    let status = resp.status().as_u16();
    let resp_headers = resp.headers().clone();
    eprintln!("[rewrite] OpenRouter response status: {}", status);

    // If OpenRouter returned an HTML error page, surface a clean JSON error
    let is_html = resp_headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| ct.contains("text/html"))
        .unwrap_or(false);
    if is_html {
        eprintln!("[rewrite] OpenRouter returned HTML (error page) — converting to 502");
        return Ok(error_response(502,
            r#"{"type":"error","error":{"type":"api_error","message":"OpenRouter returned an HTML error page"}}"#,
        ));
    }

    // Stream the response body — map reqwest chunks into hyper Frame<Bytes>
    let byte_stream = resp
        .bytes_stream()
        .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })
        .map_ok(Frame::data);

    let stream_body = StreamBody::new(byte_stream).boxed();

    let mut builder = Response::builder().status(status);
    for (key, value) in resp_headers.iter() {
        // Strip content-length — we don't know the final size when streaming
        // and Claude Code handles chunked / no content-length fine.
        if key != "content-length" && key != "transfer-encoding" {
            builder = builder.header(key, value);
        }
    }
    // Do NOT set content-length — let hyper use chunked transfer encoding
    Ok(builder.body(stream_body).unwrap())
}

/// Start the model-rewrite proxy on `listen_port`, forwarding to `upstream_port`.
/// When `openrouter_api_key` is non-empty, routes to OpenRouter instead of copilot-api.
pub async fn start(
    upstream_port: u16,
    listen_port: u16,
    openrouter_api_key: String,
) -> Result<tauri::async_runtime::JoinHandle<()>, String> {
    let state = Arc::new(ProxyState {
        upstream: format!("http://localhost:{}", upstream_port),
        openrouter_api_key: Arc::new(tokio::sync::RwLock::new(openrouter_api_key)),
    });

    let (ready_tx, ready_rx) = oneshot::channel::<Result<(), String>>();

    let handle = tauri::async_runtime::spawn(async move {
        let addr = SocketAddr::from(([127, 0, 0, 1], listen_port));
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                let msg = format!("Failed to bind port {}: {}", listen_port, e);
                eprintln!("[rewrite] {}", msg);
                let _ = ready_tx.send(Err(msg));
                return;
            }
        };

        eprintln!(
            "[rewrite] Model-rewrite proxy listening on :{} → http://localhost:{}",
            listen_port, upstream_port
        );

        let _ = ready_tx.send(Ok(()));

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
                    .ignore_invalid_headers(true)
                    .serve_connection(
                        io,
                        service_fn(move |req| {
                            let state = state.clone();
                            handle_request(state, req)
                        }),
                    )
                    .await
                {
                    let msg = e.to_string();
                    if !msg.contains("unexpected header") && !msg.contains("header name") {
                        eprintln!("[rewrite] serve error: {}", e);
                    }
                }
            });
        }
    });

    match ready_rx.await {
        Ok(Ok(())) => Ok(handle),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("Model rewriter task exited before signaling readiness".into()),
    }
}

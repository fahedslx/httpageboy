# HTTPageboy

Minimal HTTP server package for handling request/response transmission.
Focuses only on transporting a well formed HTTP message; does not process or decide how the server behaves.
Aspires to become runtime-agnostic, with minimal, solid, and flexible dependencies.

## Example

The core logic resides in `src/lib.rs`.

### See it working out of the box on [this video](https://www.youtube.com/watch?v=VwRYWJ33C4o)

The following example is executable. Run `cargo run` to see the available variants and navigate to [http://127.0.0.1:7878](http://127.0.0.1:7878) in your browser.

A basic server setup (select a runtime feature when running, e.g. `cargo run --features async_tokio`):

```rust
#![cfg(feature = "async_tokio")]
use httpageboy::{Rt, Response, Server, StatusCode};

/// Minimal async handler: waits 100ms and replies "ok"
async fn demo(_req: &()) -> Response {
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: "text/plain".into(),
    content: b"ok".to_vec(),
  }
}

#[tokio::main]
async fn main() {
  let mut srv = Server::new("127.0.0.1:7878", None).await.unwrap();
  srv.add_route("/", Rt::GET, handler!(demo));
  srv.run().await;
}
````

## Testing

Test helpers live in `httpageboy::test_utils` and work the same for sync and async runtimes:
- `setup_test_server(server_url, factory)` starts a server once per URL and marks it active (pass `None` to reuse the default `127.0.0.1:0` and let the OS pick a port).
- `run_test(request, expected, target_url)` opens a TCP connection to the active server (or the URL you pass), writes a raw HTTP payload, and asserts the response contains the expected bytes.

Async tokio example mirroring the current helpers:

```rust
#![cfg(feature = "async_tokio")]
use httpageboy::test_utils::{run_test, setup_test_server};
use httpageboy::{handler, Request, Response, Rt, Server, StatusCode};

async fn server_factory() -> Server {
  let mut server = Server::new("127.0.0.1:0", None).await.unwrap();
  server.add_route("/", Rt::GET, handler!(home));
  server
}

async fn home(_req: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: "text/plain".into(),
    content: b"home".to_vec(),
  }
}

#[tokio::test]
async fn test_home_ok() {
  setup_test_server(None, || server_factory()).await;
  let body = run_test(b"GET / HTTP/1.1\r\n\r\n", b"home", None).await;
  assert!(body.contains("home"));
}
```

## CORS

Servers now ship with a permissive CORS policy by default (allow all origins, methods, and common headers). You can tighten it after constructing the server:

```rust
let mut server = Server::new("127.0.0.1:7878", None).await.unwrap();
server.set_cors_str("origin=http://localhost:3000,credentials=true,headers=Content-Type");
// or build it directly:
// server.set_cors(CorsPolicy::from_config_str("origin=http://localhost:3000"));
```

Preflights (OPTIONS) are answered automatically using the active policy.

Comandos:

```bash
cargo test --features sync --test test_sync
cargo test --features async_tokio --test test_async_tokio
cargo test --features async_std --test test_async_std
cargo test --features async_smol --test test_async_smol
```

## Examples

Additional examples can be found within the tests.

## License

Copyright (c) 2025 [fahedsl](https://gitlab.com/fahedsl).
This project is licensed under the [MIT License](https://opensource.org/licenses/MIT).

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
use httpageboy::{route, Request, Rt, Response, Server, StatusCode};

/// Minimal async handler: waits 100ms and replies "ok"
async fn demo(_req: &Request) -> Response {
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: b"ok".to_vec(),
  }
}

#[tokio::main]
async fn main() {
  let mut srv = Server::new("127.0.0.1:7878").await.unwrap();
  srv.routes([route!("/", Rt::GET, demo)]);
  srv.run().await;
}
```

Routes can be registered together without changing the underlying router:

```rust
server.routes([
  route!("/", Rt::GET, home),
  route!("/users", Rt::GET, list_users),
  route!("/users", Rt::POST, create_user),
  route!("/search", Rt::QUERY, search),
]);
```

`QUERY` is supported as defined by RFC 10008 and may carry request content while remaining safe and idempotent.

## Protocol extensions

PageBoy can hand a routed HTTP connection to a separate protocol crate without implementing that protocol itself.

WSPageboy uses this hook for WebSocket:

```toml
[dependencies]
httpageboy = { version = "2.1.0", features = ["async_tokio"] }
wspageboy = { version = "0.1.0", features = ["async_tokio"] }
```

```rust
use httpageboy::{route, Rt, Server};
use wspageboy::WebSocket;

server.routes([
  route!("/api", Rt::GET, api),
  route!("/ws", Rt::GET, socket, WebSocket),
]);
```

The fourth argument is supplied by the extension crate. PageBoy handles the HTTP request and only hands over the connection after the extension returns `101 Switching Protocols`.

Use the matching feature in both crates: `sync`, `async_tokio`, `async_std`, or `async_smol`. PageBoy does not depend on WSPageboy.

Response now supports arbitrary headers:

```rust
Response {
  status: StatusCode::Ok.to_string(),
  headers: vec![("Content-Type".into(), "application/json".into())],
  body: br#"{"ok":true}"#.to_vec(),
}

Response {
  status: StatusCode::TemporaryRedirect.to_string(),
  headers: vec![
    ("Location".into(), "https://example.com".into()),
    ("Content-Type".into(), "text/plain".into()),
  ],
  body: Vec::new(),
}
```

## Testing

Test helpers live in `httpageboy::test_utils` and work the same for sync and async runtimes:
- `setup_test_server(server_url, factory)` starts a server once per URL and marks it active (pass `None` to reuse the default `127.0.0.1:0` and let the OS pick a port).
- `run_test(request, expected, target_url)` opens a TCP connection to the active server (or the URL you pass), writes a raw HTTP payload, and asserts the response contains the expected bytes.
- `test_case!` groups ordered setup, one or more HTTP assertions, and cleanup while still using `cargo test` as the runner.

Minimal sync lifecycle example:

```rust
#![cfg(feature = "sync")]
use httpageboy::test_utils::{setup_test_server, shutdown_test_server, TestResult};
use httpageboy::{Request, Response, Rt, Server, StatusCode, route, test_case};

const TEST_URL: &str = "127.0.0.1:0";

fn server_factory() -> Server {
  let mut server = Server::new(TEST_URL, 10).unwrap();
  server.routes([route!("/", Rt::GET, home)]);
  server
}

fn home(_req: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: b"home".to_vec(),
  }
}

#[test]
fn home_works() -> TestResult {
  test_case! {
    before {
      setup_test_server(Some(TEST_URL), || server_factory())?;
    }
    before_each {
      // Runs before each client.run(...).
    }
    test |client| {
      client.run(b"GET / HTTP/1.1\r\n\r\n", b"200 OK")?;
      client.run(b"GET / HTTP/1.1\r\n\r\n", b"home")?;
    }
    after_each {
      // Runs after each client.run(...), even if that request fails.
    }
    after {
      shutdown_test_server(TEST_URL)?;
    }
  }
}
```

Minimal tokio lifecycle example:

```rust
#![cfg(feature = "async_tokio")]
use httpageboy::test_utils::{setup_test_server, shutdown_test_server, TestResult};
use httpageboy::{Request, Response, Rt, Server, StatusCode, route, test_case};

const TEST_URL: &str = "127.0.0.1:0";

async fn server_factory() -> Server {
  let mut server = Server::new(TEST_URL).await.unwrap();
  server.routes([route!("/", Rt::GET, home)]);
  server
}

async fn home(_req: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: b"home".to_vec(),
  }
}

#[tokio::test]
async fn home_works() -> TestResult {
  test_case! {
    before {
      setup_test_server(Some(TEST_URL), || server_factory()).await?;
    }
    test |client| {
      client.run(b"GET / HTTP/1.1\r\n\r\n", b"200 OK").await?;
      client.run(b"GET / HTTP/1.1\r\n\r\n", b"home").await?;
    }
    after {
      shutdown_test_server(TEST_URL)?;
    }
  }
}
```

## CORS

Servers now ship with a permissive CORS policy by default (allow all origins, methods, and common headers). You can tighten it after constructing the server:

```rust
let mut server = Server::new("127.0.0.1:7878").await.unwrap();
server.set_cors_str("origin=http://localhost:3000,credentials=true,headers=Content-Type");
// or build it directly:
// server.set_cors(CorsPolicy::from_config_str("origin=http://localhost:3000"));
```

Preflights (OPTIONS) are answered automatically using the active policy.

## OpenAPI helper

`cargo openapi` generates OpenAPI 3.2.1 directly from implemented `route!(...)` entries. Put `// openapi:` comments immediately above the route they describe. It is dependency-free and works offline.

```rust
server.routes([
  // openapi: List users in the business
  // auth: user-token, business-id, app-id
  // permission: users.read
  // response: 200 User list
  route!("/businesses/{id}/users", Rt::GET, list_business_users),
]);
```

Default project flow:

```bash
cargo openapi
```

That reads `src/` and writes both:

```txt
docs/openapi.yaml
public/openapi.yaml
```

For custom paths:

```bash
cargo run --bin openapi_from_code -- src docs/openapi.yaml public/openapi.yaml
```

Supported route comments:

```txt
openapi: human summary
auth: user-token, business-id, app-id
headers: service-token
permission: users.read
request: json
response: 200 OK
errors: invalid_token, insufficient_permissions
```

Notes for API authors:

- Put comments immediately above the `route!(...)` call they describe.
- Use `openapi:` for the human description; without it, the handler name is used.
- Use `auth:` or `headers:` for required headers; omit it for public routes.
- Use `permission:` when the route requires an authorization permission.
- Use `request:` when the route expects a JSON body.
- Use `response:` for the main success response.
- Use `errors:` for known business error names.

The generator extracts only routes registered in code. Missing or incomplete comments are not blockers: generation continues with the route method, path, handler name, path parameters, and any comments that are present.

Comandos:

```bash
cargo test --features sync --test test_sync
cargo test --features async_tokio --test test_async_tokio
cargo test --features async_std --test test_async_std
cargo test --features async_smol --test test_async_smol
cargo test --bin openapi_from_code
```

## Examples

Additional examples can be found within the tests.

## License

Copyright (c) 2025 [fahedsl](https://gitlab.com/fahedsl).
This project is licensed under the [MIT License](https://opensource.org/licenses/MIT).

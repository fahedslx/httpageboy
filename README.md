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
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: b"ok".to_vec(),
  }
}

#[tokio::main]
async fn main() {
  let mut srv = Server::new("127.0.0.1:7878", None).await.unwrap();
  srv.add_route("/", Rt::GET, handler!(demo));
  srv.run().await;
}
````

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
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: b"home".to_vec(),
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

## OpenAPI helper

`cargo openapi` generates OpenAPI directly from implemented `server.add_route(...)` calls and the `// openapi:` comments placed immediately above each route. It is dependency-free and works offline.

```rust
// openapi: List users in the business
// auth: user-token, business-id, app-id
// permission: users.read
// response: 200 User list
server.add_route("/businesses/{id}/users", Rt::GET, handler!(list_business_users));
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

- Put comments immediately above the `server.add_route(...)` call they describe.
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

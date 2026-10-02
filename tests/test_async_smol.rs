#![cfg(feature = "async_smol")]

mod support;
use support::{TestResult, run_test, setup_test_server};
use httpageboy::{Request, Response, Rt, Server, StatusCode, route};
use std::collections::BTreeMap;
use httpageboy_test_macros::smol_test;

const REGULAR_SERVER_URL: &str = "127.0.0.1:28080";
const STRICT_SERVER_URL: &str = "127.0.0.1:28081";

async fn common_server_definition(server_url: &str) -> Server {
  let mut server = match Server::new(server_url).await {
    Ok(server) => server,
    Err(_) => Server::new("127.0.0.1:0")
      .await
      .expect("failed to bind test server"),
  };
  server.set_idle_timeout(std::time::Duration::from_millis(50));
  server.set_req_timeout(std::time::Duration::from_millis(100));
  server.routes([
    route!("/", Rt::GET, demo_handle_home),
    route!("/test", Rt::GET, demo_handle_get),
    route!("/test", Rt::POST, demo_handle_post),
    route!("/test/{param1}", Rt::POST, demo_handle_post),
    route!("/test/{param1}/{param2}", Rt::POST, demo_handle_post),
    route!("/test", Rt::PUT, demo_handle_put),
    route!("/test", Rt::PATCH, demo_handle_put),
    route!("/test", Rt::DELETE, demo_handle_delete),
    route!("/test", Rt::HEAD, demo_handle_head),
    route!("/test", Rt::OPTIONS, demo_handle_options),
    route!("/test", Rt::CONNECT, demo_handle_connect),
    route!("/test", Rt::TRACE, demo_handle_trace),
    route!("/query", Rt::QUERY, demo_handle_query),
    route!("/redirect", Rt::GET, demo_handle_redirect),
    route!("/json", Rt::GET, demo_handle_json),
    route!("/custom-header", Rt::GET, demo_handle_custom_header),
  ]);
  let res_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("res");
  server.add_files_source(res_path.to_str().unwrap());
  server
}

async fn regular_server_definition() -> Server {
  common_server_definition(REGULAR_SERVER_URL).await
}

async fn strict_server_definition() -> Server {
  common_server_definition(STRICT_SERVER_URL).await
}

async fn create_test_server() -> Server {
  regular_server_definition().await
}

async fn boot_regular() {
  if let Err(err) = setup_test_server(Some(REGULAR_SERVER_URL), || create_test_server()).await {
    panic!("{}", err);
  }
}

async fn boot_strict() {
  if let Err(err) = setup_test_server(Some(STRICT_SERVER_URL), || strict_server_definition()).await {
    panic!("{}", err);
  }
}

async fn run_regular(request: &[u8], expected: &[u8]) -> String {
  match run_test(request, expected, Some(REGULAR_SERVER_URL)).await {
    Ok(response) => response,
    Err(err) => panic!("{}", err),
  }
}

async fn run_strict(request: &[u8], expected: &[u8]) -> String {
  match run_test(request, expected, Some(STRICT_SERVER_URL)).await {
    Ok(response) => response,
    Err(err) => panic!("{}", err),
  }
}

async fn demo_handle_home(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"home".into(),
  }
}

async fn demo_handle_post(_request: &Request) -> Response {
  let mut ordered: BTreeMap<&String, &String> = BTreeMap::new();
  for (k, v) in &_request.params {
    ordered.insert(k, v);
  }
  let body = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, ordered, _request.body_text().unwrap_or("")
  );
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: body.into(),
  }
}

async fn demo_handle_get(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"get".into(),
  }
}

async fn demo_handle_put(_request: &Request) -> Response {
  let body = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, _request.params, _request.body_text().unwrap_or("")
  );
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: body.into(),
  }
}

async fn demo_handle_delete(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"delete".into(),
  }
}

async fn demo_handle_head(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"head".into(),
  }
}

async fn demo_handle_options(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"options".into(),
  }
}

async fn demo_handle_connect(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"connect".into(),
  }
}

async fn demo_handle_trace(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: b"trace".into(),
  }
}

async fn demo_handle_query(request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![],
    body: format!("query:{}", request.body_text().unwrap_or("")).into(),
  }
}

async fn demo_handle_redirect(_request: &Request) -> Response {
  Response {
    status: StatusCode::TemporaryRedirect,
    headers: vec![
      ("Location".to_string(), "https://example.com".to_string()),
      ("Content-Type".to_string(), "text/plain".to_string()),
    ],
    body: Vec::new().into(),
  }
}

async fn demo_handle_json(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![("Content-Type".to_string(), "application/json".to_string())],
    body: br#"{"ok":true}"#.into(),
  }
}

async fn demo_handle_custom_header(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok,
    headers: vec![
      ("Content-Type".to_string(), "text/plain".to_string()),
      ("X-Trace-Id".to_string(), "abc-123".to_string()),
    ],
    body: b"custom".into(),
  }
}

#[smol_test]
async fn test_home() {
  boot_regular().await;
  let request = b"GET / HTTP/1.1\r\n\r\n";
  let expected = b"home";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get_with_query() {
  boot_regular().await;
  let request = b"GET /test?foo=bar&baz=qux HTTP/1.1\r\n\r\n";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get_no_content_length() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 1\r\n\r\npong";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_get_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nhi";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_without_content_length_empty_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\n";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_query() {
  boot_regular().await;
  let request = b"POST /test?foo=bar HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {\"foo\": \"bar\"}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 15\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_params() {
  boot_regular().await;
  let request = b"POST /test/hola/que?param4=hoy&param3=hace HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected =
    b"Method: POST\nUri: /test/hola/que\nParams: {\"param1\": \"hola\", \"param2\": \"que\", \"param3\": \"hace\", \"param4\": \"hoy\"}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_incomplete_path_params() {
  boot_regular().await;
  let request = b"POST /test/hola HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test/hola\nParams: {\"param1\": \"hola\"}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_without_content_length_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_matching_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_smaller_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 2\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"bo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_post_with_larger_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nbody";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_put() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_put_without_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_put_with_matching_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 3\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_put_with_smaller_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"p\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_put_with_larger_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 8\r\n\r\nput";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_patch() {
  boot_regular().await;
  let request = b"PATCH /test HTTP/1.1\r\n\r\npatch";
  let expected = b"Method: PATCH\nUri: /test\nParams: {}\nBody: \"patch\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_head() {
  boot_regular().await;
  let request = b"HEAD /test HTTP/1.1\r\n\r\n";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  let response = run_regular(request, b"HTTP/1.1 200 OK").await;
  assert!(!response.ends_with("\r\n\r\nhead"), "{response}");
}

#[smol_test]
async fn test_options() {
  boot_regular().await;
  let request = b"OPTIONS /test HTTP/1.1\r\n\r\n";
  let expected = b"options";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_connect() {
  boot_regular().await;
  let request = b"CONNECT /test HTTP/1.1\r\n\r\n";
  let expected = b"connect";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_trace() {
  boot_regular().await;
  let request = b"TRACE /test HTTP/1.1\r\n\r\n";
  let expected = b"trace";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_query_with_body() {
  boot_regular().await;
  let request = b"QUERY /query HTTP/1.1\r\nContent-Length: 5\r\n\r\nhello";
  let expected = b"query:hello";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_query_without_content_length() {
  boot_regular().await;
  let request = b"QUERY /query HTTP/1.1\r\n\r\nhello";
  let expected = b"query:hello";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_delete() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_delete_no_content_length() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_delete_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"delete";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_delete_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nping";
  let expected = b"delete";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_delete_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 20\r\n\r\nping";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_strict_mode_without_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[smol_test]
async fn test_strict_mode_with_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 7\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[smol_test]
async fn test_strict_mode_get_without_content_length() {
  boot_strict().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[smol_test]
async fn test_file_exists() {
  boot_regular().await;
  let request = b"GET /numano.png HTTP/1.1\r\nHost: localhost\r\n\r\n";
  let expected = b"HTTP/1.1 200 OK";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_file_not_found() {
  boot_regular().await;
  let request = b"GET /test.png HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_method_not_allowed() {
  boot_regular().await;
  let request = b"BREW /coffee HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 405 Method Not Allowed";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_allowed_method_missing_route() {
  boot_regular().await;
  let request = b"TRACE /missing HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_empty_request() {
  boot_regular().await;
  let request = b"";
  let expected = b"";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_malformed_request() {
  boot_regular().await;
  let request = b"THIS_IS_NOT_HTTP\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_unsupported_http_version() {
  boot_regular().await;
  let request = b"GET / HTTP/0.9\r\n\r\n";
  let expected = b"HTTP/1.1 505 HTTP Version Not Supported";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_long_path() {
  boot_regular().await;
  let long_path = "/".to_string() + &"a".repeat(10_000);
  let request = format!("GET {} HTTP/1.1\r\n\r\n", long_path);
  let expected = b"HTTP/1.1 414 URI Too Long";
  run_regular(request.as_bytes(), expected).await;
}

#[smol_test]
async fn test_missing_method() {
  boot_regular().await;
  let request = b"/ HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  smol::Timer::after(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[smol_test]
async fn test_redirect_with_location_header() -> TestResult {
  boot_regular().await;
  let response = run_regular(b"GET /redirect HTTP/1.1\r\n\r\n", b"HTTP/1.1 307 Temporary Redirect").await;
  assert!(
    response.contains("Location: https://example.com"),
    "missing Location header: {}",
    response
  );
  assert!(
    response.contains("Content-Length: 0"),
    "wrong Content-Length for redirect: {}",
    response
  );
  Ok(())
}

#[smol_test]
async fn test_json_content_type_header() -> TestResult {
  boot_regular().await;
  let response = run_regular(b"GET /json HTTP/1.1\r\n\r\n", br#"{"ok":true}"#).await;
  assert!(
    response.contains("Content-Type: application/json"),
    "missing JSON content type: {}",
    response
  );
  assert!(
    response.contains("Content-Length: 11"),
    "wrong Content-Length for JSON: {}",
    response
  );
  Ok(())
}

#[smol_test]
async fn test_custom_header_is_serialized() -> TestResult {
  boot_regular().await;
  let response = run_regular(b"GET /custom-header HTTP/1.1\r\n\r\n", b"custom").await;
  assert!(
    response.contains("X-Trace-Id: abc-123"),
    "missing custom header: {}",
    response
  );
  assert!(
    response.contains("Content-Type: text/plain"),
    "missing Content-Type: {}",
    response
  );
  Ok(())
}

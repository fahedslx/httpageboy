#![cfg(feature = "async_tokio")]

use httpageboy::test_utils::{
  NamedTest, SuiteEvent, TestError, TestResult, TestSuite, is_test_server_registered, run_test, run_test_suite,
  setup_test_server,
};
use httpageboy::{Request, Response, Rt, Server, StatusCode, handler};
use std::collections::BTreeMap;

const REGULAR_SERVER_URL: &str = "127.0.0.1:48080";
const STRICT_SERVER_URL: &str = "127.0.0.1:48081";
const SUITE_SERVER_URL: &str = "127.0.0.1:48082";

async fn common_server_definition(server_url: &str) -> Server {
  let mut server = match Server::new(server_url, None).await {
    Ok(server) => server,
    Err(_) => Server::new("127.0.0.1:0", None)
      .await
      .expect("failed to bind test server"),
  };
  server.add_route("/", Rt::GET, handler!(demo_handle_home));
  server.add_route("/test", Rt::GET, handler!(demo_handle_get));
  server.add_route("/test", Rt::POST, handler!(demo_handle_post));
  server.add_route("/test/{param1}", Rt::POST, handler!(demo_handle_post));
  server.add_route("/test/{param1}/{param2}", Rt::POST, handler!(demo_handle_post));
  server.add_route("/test", Rt::PUT, handler!(demo_handle_put));
  server.add_route("/test", Rt::PATCH, handler!(demo_handle_put));
  server.add_route("/test", Rt::DELETE, handler!(demo_handle_delete));
  server.add_route("/test", Rt::HEAD, handler!(demo_handle_head));
  server.add_route("/test", Rt::OPTIONS, handler!(demo_handle_options));
  server.add_route("/test", Rt::CONNECT, handler!(demo_handle_connect));
  server.add_route("/test", Rt::TRACE, handler!(demo_handle_trace));
  server.add_route("/redirect", Rt::GET, handler!(demo_handle_redirect));
  server.add_route("/json", Rt::GET, handler!(demo_handle_json));
  server.add_route("/custom-header", Rt::GET, handler!(demo_handle_custom_header));
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
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"home".to_vec(),
  }
}

async fn demo_handle_post(_request: &Request) -> Response {
  let mut ordered: BTreeMap<&String, &String> = BTreeMap::new();
  for (k, v) in &_request.params {
    ordered.insert(k, v);
  }
  let body = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, ordered, _request.body
  );
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: body.into_bytes(),
  }
}

async fn demo_handle_get(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"get".to_vec(),
  }
}

async fn demo_handle_put(_request: &Request) -> Response {
  let body = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, _request.params, _request.body
  );
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: body.into_bytes(),
  }
}

async fn demo_handle_delete(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"delete".to_vec(),
  }
}

async fn demo_handle_head(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"head".to_vec(),
  }
}

async fn demo_handle_options(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"options".to_vec(),
  }
}

async fn demo_handle_connect(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"connect".to_vec(),
  }
}

async fn demo_handle_trace(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: b"trace".to_vec(),
  }
}

async fn demo_handle_redirect(_request: &Request) -> Response {
  Response {
    status: StatusCode::TemporaryRedirect.to_string(),
    headers: vec![
      ("Location".to_string(), "https://example.com".to_string()),
      ("Content-Type".to_string(), "text/plain".to_string()),
    ],
    body: Vec::new(),
  }
}

async fn demo_handle_json(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![("Content-Type".to_string(), "application/json".to_string())],
    body: br#"{"ok":true}"#.to_vec(),
  }
}

async fn demo_handle_custom_header(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![
      ("Content-Type".to_string(), "text/plain".to_string()),
      ("X-Trace-Id".to_string(), "abc-123".to_string()),
    ],
    body: b"custom".to_vec(),
  }
}

#[tokio::test]
async fn test_home() {
  boot_regular().await;
  let request = b"GET / HTTP/1.1\r\n\r\n";
  let expected = b"home";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get_with_query() {
  boot_regular().await;
  let request = b"GET /test?foo=bar&baz=qux HTTP/1.1\r\n\r\n";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get_no_content_length() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 1\r\n\r\npong";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_get_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nhi";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_without_content_length_empty_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\n";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_query() {
  boot_regular().await;
  let request = b"POST /test?foo=bar HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {\"foo\": \"bar\"}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 15\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_params() {
  boot_regular().await;
  let request = b"POST /test/hola/que?param4=hoy&param3=hace HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected =
    b"Method: POST\nUri: /test/hola/que\nParams: {\"param1\": \"hola\", \"param2\": \"que\", \"param3\": \"hace\", \"param4\": \"hoy\"}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_incomplete_path_params() {
  boot_regular().await;
  let request = b"POST /test/hola HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test/hola\nParams: {\"param1\": \"hola\"}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_without_content_length_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_matching_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_smaller_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 2\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"bo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_post_with_larger_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nbody";
  let expected = b"HTTP/1.1 200 OK";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_put() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_put_without_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_put_with_matching_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 3\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_put_with_smaller_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"p\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_put_with_larger_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 8\r\n\r\nput";
  let expected = b"HTTP/1.1 200 OK";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_patch() {
  boot_regular().await;
  let request = b"PATCH /test HTTP/1.1\r\n\r\npatch";
  let expected = b"Method: PATCH\nUri: /test\nParams: {}\nBody: \"patch\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_head() {
  boot_regular().await;
  let request = b"HEAD /test HTTP/1.1\r\n\r\n";
  let expected = b"head";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_options() {
  boot_regular().await;
  let request = b"OPTIONS /test HTTP/1.1\r\n\r\n";
  let expected = b"options";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_connect() {
  boot_regular().await;
  let request = b"CONNECT /test HTTP/1.1\r\n\r\n";
  let expected = b"connect";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_trace() {
  boot_regular().await;
  let request = b"TRACE /test HTTP/1.1\r\n\r\n";
  let expected = b"trace";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_delete() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_delete_no_content_length() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_delete_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"delete";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_delete_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nping";
  let expected = b"delete";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_delete_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 20\r\n\r\nping";
  let expected = b"delete";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_strict_mode_without_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[tokio::test]
async fn test_strict_mode_with_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 7\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[tokio::test]
async fn test_strict_mode_get_without_content_length() {
  boot_strict().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[tokio::test]
async fn test_file_exists() {
  boot_regular().await;
  let request = b"GET /numano.png HTTP/1.1\r\nHost: localhost\r\n\r\n";
  let expected = b"HTTP/1.1 200 OK";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_file_not_found() {
  boot_regular().await;
  let request = b"GET /test.png HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_method_not_allowed() {
  boot_regular().await;
  let request = b"BREW /coffee HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 405 Method Not Allowed";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_allowed_method_missing_route() {
  boot_regular().await;
  let request = b"TRACE /missing HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_empty_request() {
  boot_regular().await;
  let request = b"";
  let expected = b"HTTP/1.1 400 Bad Request";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_malformed_request() {
  boot_regular().await;
  let request = b"THIS_IS_NOT_HTTP\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_unsupported_http_version() {
  boot_regular().await;
  let request = b"GET / HTTP/0.9\r\n\r\n";
  let expected = b"HTTP/1.1 505 HTTP Version Not Supported";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
async fn test_long_path() {
  boot_regular().await;
  let long_path = "/".to_string() + &"a".repeat(10_000);
  let request = format!("GET {} HTTP/1.1\r\n\r\n", long_path);
  let expected = b"HTTP/1.1 414 URI Too Long";
  run_regular(request.as_bytes(), expected).await;
}

#[tokio::test]
async fn test_missing_method() {
  boot_regular().await;
  let request = b"/ HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[tokio::test]
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

#[tokio::test]
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

#[tokio::test]
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

#[tokio::test]
async fn test_suite_lifecycle_accumulates_results_and_shuts_down_server() {
  use std::sync::{Arc, Mutex};

  let order = Arc::new(Mutex::new(Vec::new()));
  let urls = Arc::new(Mutex::new(Vec::new()));

  let suite = TestSuite {
    before: Some(Box::new({
      let order = Arc::clone(&order);
      move || {
        let order = Arc::clone(&order);
        Box::pin(async move {
          order.lock().unwrap().push("before");
          Ok(())
        })
      }
    })),
    before_each: Some(Box::new({
      let order = Arc::clone(&order);
      move || {
        let order = Arc::clone(&order);
        Box::pin(async move {
          order.lock().unwrap().push("before_each");
          Ok(())
        })
      }
    })),
    tests: vec![
      NamedTest {
        name: "one",
        test: Box::new({
          let order = Arc::clone(&order);
          let urls = Arc::clone(&urls);
          move || {
            let order = Arc::clone(&order);
            let urls = Arc::clone(&urls);
            Box::pin(async move {
              order.lock().unwrap().push("test_1");
              urls
                .lock()
                .unwrap()
                .push(httpageboy::test_utils::active_test_server_url().to_string());
              run_test(b"GET /test HTTP/1.1\r\n\r\n", b"get", None).await.map(|_| ())
            })
          }
        }),
      },
      NamedTest {
        name: "two",
        test: Box::new({
          let order = Arc::clone(&order);
          let urls = Arc::clone(&urls);
          move || {
            let order = Arc::clone(&order);
            let urls = Arc::clone(&urls);
            Box::pin(async move {
              order.lock().unwrap().push("test_2");
              urls
                .lock()
                .unwrap()
                .push(httpageboy::test_utils::active_test_server_url().to_string());
              Err(TestError::new("controlled failure"))
            })
          }
        }),
      },
      NamedTest {
        name: "three",
        test: Box::new({
          let order = Arc::clone(&order);
          let urls = Arc::clone(&urls);
          move || {
            let order = Arc::clone(&order);
            let urls = Arc::clone(&urls);
            Box::pin(async move {
              order.lock().unwrap().push("test_3");
              urls
                .lock()
                .unwrap()
                .push(httpageboy::test_utils::active_test_server_url().to_string());
              run_test(b"GET / HTTP/1.1\r\n\r\n", b"home", None).await.map(|_| ())
            })
          }
        }),
      },
    ],
    after_each: Some(Box::new({
      let order = Arc::clone(&order);
      move || {
        let order = Arc::clone(&order);
        Box::pin(async move {
          order.lock().unwrap().push("after_each");
          Ok(())
        })
      }
    })),
    after: Some(Box::new({
      let order = Arc::clone(&order);
      move || {
        let order = Arc::clone(&order);
        Box::pin(async move {
          order.lock().unwrap().push("after");
          Ok(())
        })
      }
    })),
  };

  let result = run_test_suite(
    Some(SUITE_SERVER_URL),
    || common_server_definition(SUITE_SERVER_URL),
    suite,
  )
  .await;

  assert_eq!(
    order.lock().unwrap().as_slice(),
    [
      "before",
      "before_each",
      "test_1",
      "after_each",
      "before_each",
      "test_2",
      "after_each",
      "before_each",
      "test_3",
      "after_each",
      "after",
    ]
  );
  let urls = urls.lock().unwrap().clone();
  assert_eq!(urls.len(), 3);
  assert!(urls.iter().all(|url| url == &urls[0]));
  assert!(result.has_failures());
  assert_eq!(result.failures().len(), 1);
  assert!(matches!(
    result.steps.iter().find(|step| step.result.is_err()).map(|step| &step.event),
    Some(SuiteEvent::Test { test }) if test == "two"
  ));
  assert!(!is_test_server_registered(SUITE_SERVER_URL));
  assert!(std::net::TcpListener::bind(SUITE_SERVER_URL).is_ok());
}

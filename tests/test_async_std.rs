#![cfg(feature = "async_std")]

use httpageboy::test_utils::{run_test, setup_test_server};
use httpageboy::{Request, Response, Rt, Server, StatusCode, handler};
use std::collections::BTreeMap;

const REGULAR_SERVER_URL: &str = "127.0.0.1:58080";
const STRICT_SERVER_URL: &str = "127.0.0.1:58081";

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
  setup_test_server(Some(REGULAR_SERVER_URL), || create_test_server()).await;
}

async fn boot_strict() {
  setup_test_server(Some(STRICT_SERVER_URL), || strict_server_definition()).await;
}

async fn run_regular(request: &[u8], expected: &[u8]) -> String {
  run_test(request, expected, Some(REGULAR_SERVER_URL)).await
}

async fn run_strict(request: &[u8], expected: &[u8]) -> String {
  run_test(request, expected, Some(STRICT_SERVER_URL)).await
}

async fn demo_handle_home(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"home".to_vec(),
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
    content_type: String::new(),
    content: body.into_bytes(),
  }
}

async fn demo_handle_get(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"get".to_vec(),
  }
}

async fn demo_handle_put(_request: &Request) -> Response {
  let body = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, _request.params, _request.body
  );
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: body.into_bytes(),
  }
}

async fn demo_handle_delete(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"delete".to_vec(),
  }
}

async fn demo_handle_head(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"head".to_vec(),
  }
}

async fn demo_handle_options(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"options".to_vec(),
  }
}

async fn demo_handle_connect(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"connect".to_vec(),
  }
}

async fn demo_handle_trace(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: b"trace".to_vec(),
  }
}

#[async_std::test]
async fn test_home() {
  boot_regular().await;
  let request = b"GET / HTTP/1.1\r\n\r\n";
  let expected = b"home";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get_with_query() {
  boot_regular().await;
  let request = b"GET /test?foo=bar&baz=qux HTTP/1.1\r\n\r\n";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get_no_content_length() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 1\r\n\r\npong";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_get_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nhi";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_without_content_length_empty_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\n";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_query() {
  boot_regular().await;
  let request = b"POST /test?foo=bar HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {\"foo\": \"bar\"}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 15\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_params() {
  boot_regular().await;
  let request = b"POST /test/hola/que?param4=hoy&param3=hace HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected =
    b"Method: POST\nUri: /test/hola/que\nParams: {\"param1\": \"hola\", \"param2\": \"que\", \"param3\": \"hace\", \"param4\": \"hoy\"}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_incomplete_path_params() {
  boot_regular().await;
  let request = b"POST /test/hola HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: POST\nUri: /test/hola\nParams: {\"param1\": \"hola\"}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_without_content_length_body() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_matching_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_smaller_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 2\r\n\r\nbody";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"bo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_post_with_larger_content_length() {
  boot_regular().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nbody";
  let expected = b"HTTP/1.1 200 OK";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_put() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_put_without_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_put_with_matching_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 3\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_put_with_smaller_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nput";
  let expected = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"p\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_put_with_larger_content_length() {
  boot_regular().await;
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 8\r\n\r\nput";
  let expected = b"HTTP/1.1 200 OK";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_patch() {
  boot_regular().await;
  let request = b"PATCH /test HTTP/1.1\r\n\r\npatch";
  let expected = b"Method: PATCH\nUri: /test\nParams: {}\nBody: \"patch\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_head() {
  boot_regular().await;
  let request = b"HEAD /test HTTP/1.1\r\n\r\n";
  let expected = b"head";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_options() {
  boot_regular().await;
  let request = b"OPTIONS /test HTTP/1.1\r\n\r\n";
  let expected = b"options";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_connect() {
  boot_regular().await;
  let request = b"CONNECT /test HTTP/1.1\r\n\r\n";
  let expected = b"connect";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_trace() {
  boot_regular().await;
  let request = b"TRACE /test HTTP/1.1\r\n\r\n";
  let expected = b"trace";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_delete() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_delete_no_content_length() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected = b"delete";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_delete_with_content_length_matching_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected = b"delete";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_delete_with_content_length_smaller_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nping";
  let expected = b"delete";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_delete_with_content_length_larger_than_body() {
  boot_regular().await;
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 20\r\n\r\nping";
  let expected = b"delete";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_strict_mode_without_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[async_std::test]
async fn test_strict_mode_with_content_length() {
  boot_strict().await;
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 7\r\n\r\npayload";
  let expected = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[async_std::test]
async fn test_strict_mode_get_without_content_length() {
  boot_strict().await;
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected = b"get";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_strict(request, expected).await;
}

#[async_std::test]
async fn test_file_exists() {
  boot_regular().await;
  let request = b"GET /numano.png HTTP/1.1\r\nHost: localhost\r\n\r\n";
  let expected = b"HTTP/1.1 200 OK";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_file_not_found() {
  boot_regular().await;
  let request = b"GET /test.png HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_method_not_allowed() {
  boot_regular().await;
  let request = b"BREW /coffee HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 405 Method Not Allowed";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_allowed_method_missing_route() {
  boot_regular().await;
  let request = b"TRACE /missing HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 404 Not Found";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_empty_request() {
  boot_regular().await;
  let request = b"";
  let expected = b"HTTP/1.1 400 Bad Request";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_malformed_request() {
  boot_regular().await;
  let request = b"THIS_IS_NOT_HTTP\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_unsupported_http_version() {
  boot_regular().await;
  let request = b"GET / HTTP/0.9\r\n\r\n";
  let expected = b"HTTP/1.1 505 HTTP Version Not Supported";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

#[async_std::test]
async fn test_long_path() {
  boot_regular().await;
  let long_path = "/".to_string() + &"a".repeat(10_000);
  let request = format!("GET {} HTTP/1.1\r\n\r\n", long_path);
  let expected = b"HTTP/1.1 414 URI Too Long";
  run_regular(request.as_bytes(), expected).await;
}

#[async_std::test]
async fn test_missing_method() {
  boot_regular().await;
  let request = b"/ HTTP/1.1\r\n\r\n";
  let expected = b"HTTP/1.1 400 Bad Request";
  async_std::task::sleep(std::time::Duration::from_millis(100)).await;
  run_regular(request, expected).await;
}

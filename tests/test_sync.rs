#![cfg(feature = "sync")]
use httpageboy::test_utils::{POOL_SIZE, run_test, setup_test_server};
use httpageboy::{Request, Response, Rt, Server, StatusCode, handler};
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

const REGULAR_SERVER_URL: &str = "127.0.0.1:38080";
const STRICT_SERVER_URL: &str = "127.0.0.1:38081";

fn common_server_definition(server_url: &str) -> Server {
  let mut server = Server::new(server_url, POOL_SIZE, None).expect("failed to bind test server");
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

fn regular_server_definition() -> Server {
  common_server_definition(REGULAR_SERVER_URL)
}

fn strict_server_definition() -> Server {
  common_server_definition(STRICT_SERVER_URL)
}

fn boot_regular() {
  setup_test_server(Some(REGULAR_SERVER_URL), || regular_server_definition());
}

fn boot_strict() {
  setup_test_server(Some(STRICT_SERVER_URL), || strict_server_definition());
}

fn run_regular(request: &[u8], expected: &[u8]) -> String {
  run_test(request, expected, Some(REGULAR_SERVER_URL))
}

fn run_strict(request: &[u8], expected: &[u8]) -> String {
  run_test(request, expected, Some(STRICT_SERVER_URL))
}

fn demo_handle_home(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "home".as_bytes().to_vec(),
  }
}

fn demo_handle_post(_request: &Request) -> Response {
  // build a BTreeMap to get params in sorted key order
  let mut ordered: BTreeMap<&String, &String> = BTreeMap::new();
  for (k, v) in &_request.params {
    ordered.insert(k, v);
  }

  let request_string = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, ordered, _request.body
  );

  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: request_string.as_bytes().to_vec(),
  }
}

fn demo_handle_get(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "get".as_bytes().to_vec(),
  }
}

fn demo_handle_put(_request: &Request) -> Response {
  let request_string = format!(
    "Method: {}\nUri: {}\nParams: {:?}\nBody: {:?}",
    _request.method, _request.path, _request.params, _request.body
  );
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: request_string.as_bytes().to_vec(),
  }
}

fn demo_handle_delete(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "delete".as_bytes().to_vec(),
  }
}

fn demo_handle_head(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "head".as_bytes().to_vec(),
  }
}

fn demo_handle_options(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "options".as_bytes().to_vec(),
  }
}

fn demo_handle_connect(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "connect".as_bytes().to_vec(),
  }
}

fn demo_handle_trace(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    content_type: String::new(),
    content: "trace".as_bytes().to_vec(),
  }
}

#[test]
fn test_home() {
  boot_regular();
  let request = b"GET / HTTP/1.1\r\n\r\n";
  let expected_response = b"home";
  run_regular(request, expected_response);
}

#[test]
fn test_get() {
  boot_regular();
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_get_with_query() {
  boot_regular();
  let request = b"GET /test?foo=bar&baz=qux HTTP/1.1\r\n\r\n";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_get_no_content_length() {
  boot_regular();
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_get_with_content_length_matching_body() {
  boot_regular();
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_get_with_content_length_smaller_than_body() {
  boot_regular();
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 1\r\n\r\npong";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_get_with_content_length_larger_than_body() {
  boot_regular();
  let request = b"GET /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nhi";
  let expected_response = b"get";
  run_regular(request, expected_response);
}

#[test]
fn test_post() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_without_content_length_client_keeps_socket_open() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\n\r\npayload-open";
  let mut stream = TcpStream::connect(REGULAR_SERVER_URL).expect("connect to test server");
  stream.write_all(request).expect("write request");
  stream
    .set_read_timeout(Some(Duration::from_millis(500)))
    .expect("set read timeout");
  let mut buf = Vec::new();
  let mut chunk = [0u8; 1024];
  loop {
    match stream.read(&mut chunk) {
      Ok(0) => break,
      Ok(n) => buf.extend_from_slice(&chunk[..n]),
      Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => break,
      Err(e) => panic!("read error: {:?}", e),
    }
  }
  let text = String::from_utf8_lossy(&buf);
  assert!(
    text.contains("HTTP/1.1 200 OK") && text.contains("Body: \"payload-open\""),
    "response not received or missing body, got: {}",
    text
  );
}

#[test]
fn test_post_without_content_length_empty_body() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\n\r\n";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_query() {
  boot_regular();
  let request = b"POST /test?foo=bar HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected_response = b"Method: POST\nUri: /test\nParams: {\"foo\": \"bar\"}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_content_length() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 15\r\n\r\nmueve tu cuerpo";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_params() {
  boot_regular();
  let request = b"POST /test/hola/que?param4=hoy&param3=hace HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected_response =
    b"Method: POST\nUri: /test/hola/que\nParams: {\"param1\": \"hola\", \"param2\": \"que\", \"param3\": \"hace\", \"param4\": \"hoy\"}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_incomplete_path_params() {
  boot_regular();
  let request = b"POST /test/hola HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected_response = b"Method: POST\nUri: /test/hola\nParams: {\"param1\": \"hola\"}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_without_content_length_body() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\n\r\nbody";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_matching_content_length() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nbody";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"body\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_smaller_content_length() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 2\r\n\r\nbody";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"bo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_post_with_larger_content_length() {
  boot_regular();
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 10\r\n\r\nbody";
  let expected_response = b"HTTP/1.1 200 OK";
  run_regular(request, expected_response);
}

#[test]
fn test_put() {
  boot_regular();
  let request = b"PUT /test HTTP/1.1\r\n\r\nmueve tu cuerpo";
  let expected_response = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"mueve tu cuerpo\"";
  run_regular(request, expected_response);
}

#[test]
fn test_put_without_content_length() {
  boot_regular();
  let request = b"PUT /test HTTP/1.1\r\n\r\nput";
  let expected_response = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  run_regular(request, expected_response);
}

#[test]
fn test_put_with_matching_content_length() {
  boot_regular();
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 3\r\n\r\nput";
  let expected_response = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"put\"";
  run_regular(request, expected_response);
}

#[test]
fn test_put_with_smaller_content_length() {
  boot_regular();
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nput";
  let expected_response = b"Method: PUT\nUri: /test\nParams: {}\nBody: \"p\"";
  run_regular(request, expected_response);
}

#[test]
fn test_put_with_larger_content_length() {
  boot_regular();
  let request = b"PUT /test HTTP/1.1\r\nContent-Length: 8\r\n\r\nput";
  let expected_response = b"HTTP/1.1 200 OK";
  run_regular(request, expected_response);
}

#[test]
fn test_patch() {
  boot_regular();
  let request = b"PATCH /test HTTP/1.1\r\n\r\npatch";
  let expected_response = b"Method: PATCH\nUri: /test\nParams: {}\nBody: \"patch\"";
  run_regular(request, expected_response);
}

#[test]
fn test_head() {
  boot_regular();
  let request = b"HEAD /test HTTP/1.1\r\n\r\n";
  let expected_response = b"head";
  run_regular(request, expected_response);
}

#[test]
fn test_options() {
  boot_regular();
  let request = b"OPTIONS /test HTTP/1.1\r\n\r\n";
  let expected_response = b"options";
  run_regular(request, expected_response);
}

#[test]
fn test_connect() {
  boot_regular();
  let request = b"CONNECT /test HTTP/1.1\r\n\r\n";
  let expected_response = b"connect";
  run_regular(request, expected_response);
}

#[test]
fn test_trace() {
  boot_regular();
  let request = b"TRACE /test HTTP/1.1\r\n\r\n";
  let expected_response = b"trace";
  run_regular(request, expected_response);
}

#[test]
fn test_delete() {
  boot_regular();
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected_response = b"delete";
  run_regular(request, expected_response);
}

#[test]
fn test_delete_no_content_length() {
  boot_regular();
  let request = b"DELETE /test HTTP/1.1\r\n\r\n";
  let expected_response = b"delete";
  run_regular(request, expected_response);
}

#[test]
fn test_delete_with_content_length_matching_body() {
  boot_regular();
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 4\r\n\r\nping";
  let expected_response = b"delete";
  run_regular(request, expected_response);
}

#[test]
fn test_delete_with_content_length_smaller_than_body() {
  boot_regular();
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 1\r\n\r\nping";
  let expected_response = b"delete";
  run_regular(request, expected_response);
}

#[test]
fn test_delete_with_content_length_larger_than_body() {
  boot_regular();
  let request = b"DELETE /test HTTP/1.1\r\nContent-Length: 20\r\n\r\nping";
  let expected_response = b"delete";
  run_regular(request, expected_response);
}

#[test]
fn test_strict_mode_without_content_length() {
  boot_strict();
  let request = b"POST /test HTTP/1.1\r\n\r\npayload";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  run_strict(request, expected_response);
}

#[test]
fn test_strict_mode_with_content_length() {
  boot_strict();
  let request = b"POST /test HTTP/1.1\r\nContent-Length: 7\r\n\r\npayload";
  let expected_response = b"Method: POST\nUri: /test\nParams: {}\nBody: \"payload\"";
  run_strict(request, expected_response);
}

#[test]
fn test_strict_mode_get_without_content_length() {
  boot_strict();
  let request = b"GET /test HTTP/1.1\r\n\r\n";
  let expected_response = b"get";
  run_strict(request, expected_response);
}

#[test]
fn test_file_exists() {
  boot_regular();
  let request = b"GET /numano.png HTTP/1.1\r\nHost: localhost\r\n\r\n";
  let expected_response = b"HTTP/1.1 200 OK";
  run_regular(request, expected_response);
}

#[test]
fn test_file_not_found() {
  boot_regular();
  let request = b"GET /test.png HTTP/1.1\r\n\r\n";
  let expected_response = b"HTTP/1.1 404 Not Found";
  run_regular(request, expected_response);
}

#[test]
fn test_method_not_allowed() {
  boot_regular();
  let request = b"BREW /coffee HTTP/1.1\r\n\r\n";
  let expected_response = b"HTTP/1.1 405 Method Not Allowed";
  run_regular(request, expected_response);
}

#[test]
fn test_allowed_method_missing_route() {
  boot_regular();
  let request = b"TRACE /missing HTTP/1.1\r\n\r\n";
  let expected_response = b"HTTP/1.1 404 Not Found";
  run_regular(request, expected_response);
}

#[test]
fn test_empty_request() {
  boot_regular();
  let request = b"";
  let expected_response = b"HTTP/1.1 400 Bad Request";
  run_regular(request, expected_response);
}

#[test]
fn test_malformed_request() {
  boot_regular();
  let request = b"THIS_IS_NOT_HTTP\r\n\r\n";
  let expected_response = b"HTTP/1.1 400 Bad Request";
  run_regular(request, expected_response);
}

#[test]
fn test_unsupported_http_version() {
  boot_regular();
  let request = b"GET / HTTP/0.9\r\n\r\n";
  let expected_response = b"HTTP/1.1 505 HTTP Version Not Supported";
  run_regular(request, expected_response);
}

#[test]
fn test_long_path() {
  boot_regular();
  let long_path = "/".to_string() + &"a".repeat(10_000);
  let request = format!("GET {} HTTP/1.1\r\n\r\n", long_path);
  let expected_response = b"HTTP/1.1 414 URI Too Long";
  run_regular(request.as_bytes(), expected_response);
}

#[test]
fn test_missing_method() {
  boot_regular();
  let request = b"/ HTTP/1.1\r\n\r\n";
  let expected_response = b"HTTP/1.1 400 Bad Request";
  run_regular(request, expected_response);
}

#[cfg(feature = "async_tokio")]
use tokio::time::{Duration, sleep};
#[cfg(feature = "async_std")]
use {async_std::task::sleep, std::time::Duration};
#[cfg(feature = "async_smol")]
use {smol::Timer as SmolTimer, std::time::Duration};

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
use httpageboy::{Request, Response, Rt, Server, StatusCode, route};

// ROUTE HANDLER
#[cfg(feature = "sync")]
fn demo_get(_request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: "<!DOCTYPE html><html><head>\
<meta charset=\"utf-8\">\
</head><body>🤓: Hi, this is Pageboy working.
<br>Do you like the <a href=\"/HTTPageboy.svg\">new icon</a>?</body></html>"
      .as_bytes()
      .to_vec(),
  }
}

#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
async fn demo_get(_request: &Request) -> Response {
  #[cfg(feature = "async_tokio")]
  sleep(Duration::from_millis(100)).await;
  #[cfg(feature = "async_std")]
  sleep(Duration::from_millis(100)).await;
  #[cfg(feature = "async_smol")]
  SmolTimer::after(Duration::from_millis(100)).await;

  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![],
    body: "<!DOCTYPE html><html><head>\
<meta charset=\"utf-8\">\
</head><body>🤓: Hi, this is Pageboy working.
<br>Do you like the <a href=\"/HTTPageboy.svg\">new icon</a>?</body></html>"
      .as_bytes()
      .to_vec(),
  }
}


#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
fn query_response(request: &Request) -> Response {
  Response {
    status: StatusCode::Ok.to_string(),
    headers: vec![("Content-Type".into(), "text/plain".into())],
    body: format!("QUERY: {}", request.body).into_bytes(),
  }
}

#[cfg(feature = "sync")]
fn demo_query(request: &Request) -> Response {
  query_response(request)
}

#[cfg(any(feature = "async_tokio", feature = "async_std", feature = "async_smol"))]
async fn demo_query(request: &Request) -> Response {
  query_response(request)
}

#[cfg(any(
  feature = "sync",
  feature = "async_tokio",
  feature = "async_std",
  feature = "async_smol"
))]
fn configure_server(server: &mut Server) {
  server.routes([
    // openapi: PageBoy example
    // response: 200 HTML example
    route!("/", Rt::GET, demo_get),
    // openapi: Safe query with request content
    // request: string
    // response: 200 Query result
    route!("/query", Rt::QUERY, demo_query),
  ]);
  server.add_files_source("res");
}

// SYNC
#[cfg(feature = "sync")]
fn main() {
  let serving_url: &str = "0.0.0.0:7878";
  let threads_number: u8 = 10;

  let mut server = Server::new(serving_url, threads_number).unwrap();
  configure_server(&mut server);
  server.run();
}

// ASYNC TOKIO
#[cfg(all(not(feature = "sync"), feature = "async_tokio"))]
#[tokio::main]
async fn main() {
  let serving_url: &str = "0.0.0.0:7878";

  let mut server = Server::new(serving_url).await.unwrap();
  configure_server(&mut server);
  server.run().await;
}

// ASYNC STD
#[cfg(all(not(feature = "sync"), not(feature = "async_tokio"), feature = "async_std"))]
#[async_std::main]
async fn main() {
  let serving_url: &str = "0.0.0.0:7878";

  let mut server = Server::new(serving_url).await.unwrap();
  configure_server(&mut server);
  server.run().await;
}

// ASYNC SMOL
#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_std"),
  feature = "async_smol"
))]
fn main() {
  smol::block_on(run_smol());
}

#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_std"),
  feature = "async_smol"
))]
async fn run_smol() {
  let serving_url: &str = "0.0.0.0:7878";

  let mut server = Server::new(serving_url).await.unwrap();
  configure_server(&mut server);
  server.run().await;
}

// DEFAULT (NO FEATURES)
#[cfg(all(
  not(feature = "sync"),
  not(feature = "async_tokio"),
  not(feature = "async_std"),
  not(feature = "async_smol")
))]
fn main() {
  eprintln!(
    "\n❌ No feature selected. Select any of the following:\n\n  cargo run --features sync\n  cargo run --features async_tokio\n  cargo run --features async_std\n  cargo run --features async_smol\n"
  );
}

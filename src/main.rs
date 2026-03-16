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
use httpageboy::{Request, Response, Rt, Server, StatusCode, handler};

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

// SYNC
#[cfg(feature = "sync")]
fn main() {
  let serving_url: &str = "0.0.0.0:7878";
  let threads_number: u8 = 10;

  let mut server = Server::new(serving_url, threads_number, None).unwrap();
  server.add_route("/", Rt::GET, handler!(demo_get));
  server.add_files_source("res");
  server.run();
}

// ASYNC TOKIO
#[cfg(all(not(feature = "sync"), feature = "async_tokio"))]
#[tokio::main]
async fn main() {
  let serving_url: &str = "0.0.0.0:7878";

  let mut server = Server::new(serving_url, None).await.unwrap();
  server.add_route("/", Rt::GET, handler!(demo_get));
  server.add_files_source("res");
  server.run().await;
}

// ASYNC STD
#[cfg(all(not(feature = "sync"), not(feature = "async_tokio"), feature = "async_std"))]
#[async_std::main]
async fn main() {
  let serving_url: &str = "0.0.0.0:7878";

  let mut server = Server::new(serving_url, None).await.unwrap();
  server.add_route("/", Rt::GET, handler!(demo_get));
  server.add_files_source("res");
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

  let mut server = Server::new(serving_url, None).await.unwrap();
  server.add_route("/", Rt::GET, handler!(demo_get));
  server.add_files_source("res");
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

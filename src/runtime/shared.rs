pub fn print_server_info(addr: std::net::SocketAddr, _auto_close: bool) {
  // println!("Connection autoclose set to {:?}", _auto_close);

  let url = format!("http://{}", addr);
  let _green_url = format!("\x1b[32m{}\x1b[0m", url);

  #[cfg(feature = "sync")]
  println!("Serving (sync) on {}", _green_url);

  #[cfg(feature = "async_tokio")]
  println!("Serving (async_tokio) on {}", _green_url);

  #[cfg(feature = "async_std")]
  println!("Serving (async_std) on {}", _green_url);

  #[cfg(feature = "async_smol")]
  println!("Serving (async_smol) on {}", _green_url);
}

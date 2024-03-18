
use std::io;

use axum::{Router, routing::get, response::IntoResponse, extract::{State, Path}, http::StatusCode};
use color_eyre::eyre;
use serde::{Deserialize, Serialize};



#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;

  
  
  
  //http_server(()).await?;


  
  // for e in hashes.as_array().unwrap() {
  //   println!("{}", e.as_str().unwrap());
  // }

  Ok(())
}


#[derive(Debug, Deserialize, Serialize)]
struct Params {
  path: String
}

async fn get_path(State(state): State<()>, Path(params): Path<Params>) -> impl IntoResponse {
  tracing::info!(params.path, "GET");
  StatusCode::OK
}

pub async fn http_server(state: ()) -> io::Result<()> {
    let app = Router::new()
        .route("/:path", get(get_path))
        .with_state(state);


    let listener = tokio::net::TcpListener::bind("localhost:4488")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await
}

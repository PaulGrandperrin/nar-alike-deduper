use std::io;

use axum::{Router, routing::get, response::{Result, IntoResponse, ErrorResponse}, extract::{State, Path}, http::StatusCode};
use chrono::format;
use color_eyre::eyre;
use serde::{Deserialize, Serialize};

trait ResultToResponse<T> {
  fn to_500(self) -> Result<T>;
}

impl<T, E: std::error::Error> ResultToResponse<T> for std::result::Result<T, E> {
  fn to_500(self) -> Result<T> {
    self.map_err(|e| {
        tracing::error!(e = ?e, "error");
        ErrorResponse::from((StatusCode::INTERNAL_SERVER_ERROR, format!("{:?}", e)))
    })
  }
}

fn err500<T>(msg: &str) -> Result<T> {
  Err(ErrorResponse::from((StatusCode::INTERNAL_SERVER_ERROR, msg.to_string())))
}


#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;
  
  http_server(()).await?;

  Ok(())
}


#[derive(Debug, Deserialize, Serialize)]
struct Params {
  path: String
}

async fn get_path(State(state): State<()>, Path(params): Path<Params>) -> Result<impl IntoResponse> {
  tracing::info!(params.path, "GET");
  let r = reqwest::Client::new().get(format!("https://cache.nixos.org/{}", params.path)).send().await.to_500()?;
  if r.status() != 200 {
    return err500("upstream error");
  }
  Ok(r.bytes().await.to_500()?)
}

pub async fn http_server(state: ()) -> io::Result<()> {
    let app = Router::new()
        .route("/*path", get(get_path))
        .with_state(state);


    let listener = tokio::net::TcpListener::bind("localhost:4488")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await
}

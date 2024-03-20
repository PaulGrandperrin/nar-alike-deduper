use std::{io, time::Duration};

use axum::{Router, routing::get, response::{Result, IntoResponse, ErrorResponse, Response}, extract::{State, Path, MatchedPath}, http::{StatusCode, Request}};
use chrono::format;
use color_eyre::eyre;
use serde::{Deserialize, Serialize};
use tower_http::{trace::TraceLayer, classify::ServerErrorsFailureClass};
use tracing::{info_span, Span};

trait ResultToResponse<T> {
  fn to_err500(self) -> Result<T>;
}

impl<T, E: std::error::Error> ResultToResponse<T> for std::result::Result<T, E> {
  fn to_err500(self) -> Result<T> {
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
  let r = reqwest::Client::new().get(format!("https://cache.nixos.orgd/{}", params.path)).send().await.to_err500()?;
  //if r.status() != 200 {
  //  return err500("upstream error");
  //}
  Ok((StatusCode::from_u16(u16::from(r.status())).unwrap(), r.bytes().await.to_err500()?))
}

async fn nix_cache_info() -> Result<impl IntoResponse> {
  Ok("StoreDir: /nix/store
WantMassQuery: 1
Priority: 30
")
}

pub async fn http_server(state: ()) -> io::Result<()> {
    let app = Router::new()
        .route("/nix-cache-info", get(nix_cache_info))
        .route("/*path", get(get_path))
        .layer(TraceLayer::new_for_http()
          .make_span_with(|request: &Request<_>| {
            //let matched_path = request.extensions().get::<MatchedPath>().map(MatchedPath::as_str);
            info_span!(
              "http_request",
              method = %request.method(),
              //matched_path,
              uri = %request.uri(),
            )
          })
          .on_request(|_request: &Request<_>, _span: &Span| {
            tracing::info!("handling request");
          })
        )
        .with_state(state);


    let listener = tokio::net::TcpListener::bind("localhost:4489")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await
}

use std::{io, time::Duration};

use axum::{Router, routing::get, response::{IntoResponse, Response}, extract::{State, Path, MatchedPath}, http::{StatusCode, Request}};
use chrono::format;
use color_eyre::eyre;
use serde::{Deserialize, Serialize};
use tower_http::{trace::TraceLayer, classify::ServerErrorsFailureClass};
use tracing::{info_span, Span};

/// A trait to represent a response that can be returned from an HTTP handler where we can easily use `?` to return an error.
trait IntoResultReponse: IntoResponse {}
impl<T: IntoResponse> IntoResultReponse for Result<T> {}

type Result<T, E = Error> = std::result::Result<T, E>;

/// A extention trait to Result to easily convert an error into a `HttpError` with a status code.
trait ResultExt<T: IntoResponse> {
  fn err_with_status(self, status: StatusCode) -> Result<T>;
} 

impl<T: IntoResponse, E: std::fmt::Debug + 'static > ResultExt<T> for std::result::Result<T, E> {
  fn err_with_status(self, status:StatusCode) -> Result<T> {
    self.map_err(|e| {
      Error::new(status, e)
    })
  }
}

struct Error {
  status: StatusCode,
  report: Box<dyn std::fmt::Debug>,
}

impl Error {
  fn new(status: StatusCode, err: impl std::fmt::Debug + 'static) -> Self {
    Self {
      status,
      report: Box::new(err),
    }
  }
}

impl<E: std::fmt::Debug + 'static > From<E> for Error
{
  fn from(e: E) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, e)
  }
}

impl IntoResponse for Error {
  fn into_response(self) -> Response {
    let report = format!("{:?}", self.report);
    tracing::error!(error = %report);
    (self.status, report).into_response()
  }
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

async fn get_path(State(state): State<()>, Path(params): Path<Params>) -> impl IntoResultReponse { // Result<impl IntoResponse, HttpError> {
  let r = reqwest::Client::new().get(format!("https://cache.nixos.org/{}", params.path)).send().await?;
  if r.status() != 200 {
    return Err(eyre::eyre!("upstream error").into());
  }
  Ok((StatusCode::from_u16(u16::from(r.status())).unwrap(), r.bytes().await?))
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
              "request",
              method = %request.method(),
              uri = %request.uri(),
              //matched_path,
            )
          })
          .on_request(|_request: &Request<_>, _span: &Span| {
            tracing::info!("handling request");
          })
          //.on_failure(()) // TraceLayer traces failures by default but already do it manually
        )
        .with_state(state);


    let listener = tokio::net::TcpListener::bind("localhost:4489")
        .await
        .unwrap();
    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await
}

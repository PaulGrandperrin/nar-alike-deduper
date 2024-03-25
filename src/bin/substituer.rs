use std::{collections::HashMap, error::Error, io, rc::Rc, sync::Arc, time::Duration};

use axum::{Router, routing::get, response::{IntoResponse, Response}, extract::{State, Path, MatchedPath}, http::{StatusCode, Request}, body::Body};
use chrono::format;
use color_eyre::eyre::{self, anyhow};
use nar_alike_deduper::AsyncSha256Hasher;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tokio_util::io::{ReaderStream, StreamReader};
use tower_http::{trace::TraceLayer, classify::ServerErrorsFailureClass};
use tracing::{info_span, Span};

/// A trait to represent a response that can be returned from an HTTP handler where we can easily use `?` to return an error.
trait IntoResultReponse: IntoResponse {}
impl<T: IntoResponse> IntoResultReponse for Result<T> {}

type Result<T, E = HttpError> = std::result::Result<T, E>;

/// A extention trait to Result to easily convert an error into a `HttpError` with a status code.
trait ResultExt<T: IntoResponse> {
  fn err_with_status(self, status: StatusCode) -> Result<T>;
} 

impl<T: IntoResponse, E: Into<Box<dyn Error>>> ResultExt<T> for std::result::Result<T, E> {
  fn err_with_status(self, status:StatusCode) -> Result<T> {
    self.map_err(|error| {
      HttpError::new(status, error)
    })
  }
}

struct HttpError {
  status: StatusCode,
  error: Box<dyn Error>,
}

impl HttpError {
  fn new(status: StatusCode, error: impl Into<Box<dyn Error>>) -> Self {
    Self {
      status,
      error: error.into(),
    }
  }
}

impl<E: Into<Box<dyn Error>> > From<E> for HttpError
{
  fn from(error: E) -> Self {
    Self::new(StatusCode::INTERNAL_SERVER_ERROR, error)
  }
}

impl IntoResponse for HttpError {
  fn into_response(self) -> Response {
    let error = format!("{:?}", self.error);
    tracing::error!(error = %error);
    (self.status, error).into_response()
  }
}


#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;

  http_server(MyState::default()).await?;

  Ok(())
}


#[derive(Debug, Deserialize, Serialize)]
struct Params {
  path: String
}

async fn get_nar(State(state): State<MyState>, Path(params): Path<Params>) -> impl IntoResultReponse {
  if params.path.ends_with(".nar.xz") {
    let r = reqwest::Client::new().get(format!("https://cache.nixos.org/nar/{}", params.path)).send().await?;
    let status = StatusCode::from_u16(u16::from(r.status())).unwrap();
    let body = Body::from_stream(r.bytes_stream());
    Ok((status, IntoResponse::into_response(body)))
  } else if params.path.ends_with(".nar") {
    let xz_hash = state.data.read().await.get(&params.path).ok_or(anyhow!("No hash found for nar"))?.to_owned();
    let r = reqwest::Client::new().get(format!("https://cache.nixos.org/nar/{}.nar.xz", xz_hash)).send().await?;
    let status = StatusCode::from_u16(u16::from(r.status())).unwrap();



    // convert a Steamer of Bytes to an AsyncReader
    let bs = r.bytes_stream();
    let ms = bs.map(|result| result.map_err(|err| {
      std::io::Error::new(std::io::ErrorKind::Other, err)
    }));
    let sr = StreamReader::new(ms);


    let ds = async_compression::tokio::bufread::XzDecoder::new(sr);
    let s = ReaderStream::new(ds);


    let body = Body::from_stream(s);
    Ok((status, IntoResponse::into_response(body)))
  } else {
    Err(anyhow!("Only .nar.xz files are supported").into())
  }
}

async fn get_other(State(state): State<MyState>, Path(params): Path<Params>) -> impl IntoResultReponse {
  if ! params.path.ends_with(".narinfo") {
    return Err(anyhow!("Only .narinfo files are supported").into());
  }

  let r = reqwest::Client::new().get(format!("https://cache.nixos.org/{}", params.path)).send().await?;
  if ! r.status().is_success() {
    if r.status().as_u16() == StatusCode::NOT_FOUND {
      return Ok((axum::http::StatusCode::NOT_FOUND, "".into_response()));
    }
    return Err(anyhow!("Failed to fetch narinfo from cache.nixos.org").into());
  }
  let status = StatusCode::from_u16(u16::from(r.status())).unwrap();

  let text = r.text().await?; 
  let mut data = text.lines().filter_map(|l| {
    l.find(": ").map(|i| (&l[..i], l[i+2..].to_owned()))
  }).collect::<HashMap<_,_>>();


  state.data.write().await.insert(format!("{}.nar", data.get("NarHash").unwrap().split(":").nth(1).unwrap()), data.get("FileHash").unwrap().split(":").nth(1).unwrap().to_owned());

  data.remove("Compression");
  data.insert("FileHash", data.get("NarHash").unwrap().to_owned());
  data.insert("FileSize", data.get("NarSize").unwrap().to_owned());
  data.insert("URL", format!("nar/{}.nar", data.get("NarHash").unwrap().split(":").nth(1).unwrap()));

  let body = data.into_iter().map(|(k, v)| {
    format!("{}: {}", k, v)
  }).collect::<Vec<_>>().join("\n") + "\n";

  Ok((status, IntoResponse::into_response(body)))
}

async fn nix_cache_info() -> Result<impl IntoResponse> {
  Ok("StoreDir: /nix/store
WantMassQuery: 1
Priority: 30
")
}

#[derive(Debug, Clone, Default)]
struct MyState {
  data: Arc<RwLock<HashMap<String, String>>>
}


pub async fn http_server(state: MyState) -> io::Result<()> {
    let app = Router::new()
        .route("/nix-cache-info", get(nix_cache_info))
        .route("/nar/*path", get(get_nar))
        .route("/*path", get(get_other))
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

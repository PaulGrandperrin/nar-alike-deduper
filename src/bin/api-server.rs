
use std::io;

use axum::{Router, routing::get, response::IntoResponse, extract::{State, Path}, http::StatusCode};
use color_eyre::eyre;
use serde::{Deserialize, Serialize};



#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;

  //let mut decompress_from_stdin = async_compression::tokio::bufread::XzDecoder::new(tokio::io::BufReader::new(stdin()));
  //let mut compress_to_stdout = async_compression::tokio::write::XzEncoder::with_quality(stdout(), Level::Precise(9)); //6
  
  
  //let reader = File::open("file.nar").await?;
  //let mut writer = AsyncSha256Hasher::new();
  //replace_nix_paths(decompress_from_stdin, &mut compress_to_stdout).await?;
  //let hash = writer.finalize();
  //println!("{}", hex::encode(hash));
  
  //tokio::io::copy(&mut stdin(), &mut compress_to_stdout).await?;
  //compress_to_stdout.shutdown().await?;
  
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

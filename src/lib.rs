mod store_path_automaton;

use std::{borrow::Cow, pin::Pin, task::{Context, Poll}, io};
use color_eyre::eyre;
use sha2::{Sha256, Digest};
use tokio::io::{AsyncWrite, AsyncRead, AsyncReadExt, AsyncWriteExt};
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter, Registry, Layer, layer::SubscriberExt, util::SubscriberInitExt};

const REPL_PATH_LEN : usize = 44;
const CHUNK : usize = 50; // MUST be bigger than REPL_PATH_LEN

pub fn setup_logging() -> eyre::Result<()> {
  color_eyre::install()?;

  let format_layer = tracing_subscriber::fmt::layer()
    .with_span_events(FmtSpan::NONE)
    .with_filter(
      EnvFilter::from_default_env()
        .add_directive("info".parse()?)
    );
  
  let r = Registry::default()
    .with(ErrorLayer::default())
    .with(format_layer);

  #[cfg(tokio_unstable)]
  let r = r.with(console_subscriber::spawn());
  
  r.init();

  Ok(())
}

pub struct AsyncSha256Hasher {
  hasher: Sha256
}

impl AsyncSha256Hasher {
  pub fn new() -> Self {
    Self {
      hasher: Sha256::new()
    }
  }

  pub fn finalize(self) -> [u8; 32] {
    self.hasher.finalize().into()
  }
}

impl AsyncWrite for AsyncSha256Hasher {
  fn poll_write(mut self: Pin<&mut Self>, _cx: &mut Context, buf: &[u8]) -> Poll<Result<usize, io::Error>> {
    self.hasher.update(buf);
    Poll::Ready(Ok(buf.len()))
  }

  fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context) -> Poll<Result<(), io::Error>> {
    Poll::Ready(Ok(()))
  }

  fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
    Poll::Ready(Ok(()))
  }
}


pub async fn replace_nix_paths(mut reader: impl AsyncRead + Unpin, mut writer: impl AsyncWrite + Unpin) -> eyre::Result<()> {
  // the first CHUNK bytes contains the current chunk
  // the last REPL_PATH_LEN bytes contains the beginning of the next chunk
  // this is necessary to be able to search and replace over the chunk boundary
  let mut buf = [0; CHUNK + REPL_PATH_LEN ];
  // the first CHUNK bytes contains the next chunk
  // the last REPL_PATH_LEN bytes contains the begining the current chunk potentially modified by the search and replace of the previous loop cycle
  let mut buf_ahead = [0; CHUNK + REPL_PATH_LEN ];

  let mut buf_l;
  let mut buf_ahead_l;

  // nix store path with base32 hash: digit and alphabet without "eout"
  let regex = regex::bytes::Regex::new(r"/nix/store/[0-9abcdfghijklmnpqrsvwxyz]{32}\-")?;

  // read first chunk to the first part of the buffer
  // read multiple times until the chunk is filled or the end of the stream is reached (l == 0)
  buf_l = 0;
  loop {
    let l = reader.read(&mut buf[buf_l..CHUNK]).await?;
    buf_l += l;
    if l == 0 || buf_l == CHUNK {
      break;
    }
  }
  // Artifically setup the second part of the ahead buffer with the first REPL_PATH_LEN bytes from buf.
  // Those bytes are later used to carry overlapping replacements over from the last chunk to the next.
  buf_ahead[CHUNK..].copy_from_slice(&buf[..REPL_PATH_LEN]);

  loop {
    // Here, we assume that the first CHUNK bytes of buf are available
    // and that the last REPL_PATH_LEN bytes of buf_ahead contains what might need to be carried over from the search and replace of the last chunk.

    // read ahead the next chunk to the first part of the ahead buffer
    // read multiple times until the chunk is filled or the end of the stream is reached (l == 0)
    buf_ahead_l = 0;
    loop {
      let l = reader.read(&mut buf_ahead[buf_ahead_l..CHUNK]).await?;
      buf_ahead_l += l;
      if l == 0 || buf_ahead_l == CHUNK {
        break;
      }
    }
    
    // Complete the second part of the buffer with the beginning of the next chunk.
    buf[CHUNK..].copy_from_slice(&buf_ahead[..REPL_PATH_LEN]);

    // Carry over the last REPL_PATH_LEN bytes from the last chunk (saved in the the second part of the ahead buffer).
    // This is necessary because the regex replacement might overlap the chunk boundary.
    buf[..REPL_PATH_LEN].copy_from_slice(&buf_ahead[CHUNK..]);
    
    // length of available data to process
    let proc_l = buf_l + (buf_ahead_l).min(REPL_PATH_LEN);

    // actual search and replace.
    // doesn't replace in place in memory but if nothing is found, no copy is done.
    let r = regex.replace_all(&buf[..proc_l], b"/nix/store/00000000000000000000000000000000-");
    if let Cow::Owned(v) = r {
      buf[..proc_l].copy_from_slice(&v);
    }

    if buf_ahead_l == CHUNK { // if there's more chunks to read
      writer.write_all(&buf[..CHUNK]).await?; // write one chunk
    } else {
      writer.write_all(&buf[..proc_l]).await?; // write everything that's been processed. might be smaller or bigger than CHUNK
      break
    };

    // swap buffers and their respective lengths
    std::mem::swap(&mut buf, &mut buf_ahead);
    std::mem::swap(&mut buf_l, &mut buf_ahead_l);
  }

  Ok(())
}


#[cfg(test)]
mod tests {
  use super::*;
  use tokio::io::{AsyncRead, AsyncWrite};
  use std::io::Cursor;

  fn helper() {

  }

  #[tokio::test]
  async fn test_replace_nix_paths() -> eyre::Result<()> {
    let mut r = Cursor::new(b"/nix/store/abcdfghijklmnpqrsvwxyz0000000000-");
    let mut w = Vec::new();
    replace_nix_paths(&mut r, &mut w).await?;
    assert_eq!(w, b"/nix/store/00000000000000000000000000000000-");
    Ok(())
  }
}

use color_eyre::eyre;
use sha2::{Sha256, Digest};
use tokio::{fs::File, io::{AsyncRead, AsyncWrite, AsyncReadExt, AsyncWriteExt}};
use std::{borrow::Cow::Owned, pin::Pin, task::{Context, Poll}, io};

const REPL_PATH_LEN : usize = 44;
const CHUNK : usize = 50; // MUST be bigger than REPL_PATH_LEN

#[tokio::main]
async fn main() -> eyre::Result<()> {
  let reader = File::open("file.nar").await?;
  let mut writer = AsyncSha256Hasher::new();
  replace_nix_paths(reader, &mut writer).await?;
  let hash = writer.finalize();
  println!("{}", hex::encode(hash));
  Ok(())
}

struct AsyncSha256Hasher {
  hasher: Sha256
}

impl AsyncSha256Hasher {
  fn new() -> Self {
    Self {
      hasher: Sha256::new()
    }
  }

  fn finalize(self) -> [u8; 32] {
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


async fn replace_nix_paths(mut reader: impl AsyncRead + Unpin, mut writer: impl AsyncWrite + Unpin) -> eyre::Result<()> {
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
  buf_l = reader.read(&mut buf[..CHUNK]).await?;
  // Artifically setup the second part of the ahead buffer with the first REPL_PATH_LEN bytes from buf.
  // Those bytes are later used to carry overlapping replacements over from the last chunk to the next.
  buf_ahead[CHUNK..].copy_from_slice(&buf[..REPL_PATH_LEN]);

  loop {
    // Here, we assume that the first CHUNK bytes of buf are available
    // and that the last REPL_PATH_LEN bytes of buf_ahead contains what might need to be carried over from the search and replace of the last chunk.

    // read ahead the next chunk to the first part of the ahead buffer
    buf_ahead_l = reader.read(&mut buf_ahead[..CHUNK]).await?;
    
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
    if let Owned(v) = r {
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

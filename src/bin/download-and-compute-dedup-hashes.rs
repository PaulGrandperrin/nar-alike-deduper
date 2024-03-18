use async_channel::Receiver;
use color_eyre::eyre;
use nar_alike_deduper::AsyncSha256Hasher;
use sqlx::{postgres::{PgPoolOptions, PgTypeInfo, types::Oid}, Row, Column};
use futures::{TryStreamExt, StreamExt};
use tokio_util::io::StreamReader;
use std::{fmt::Write, collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Arc}};
          
async fn process_hash2(client: &reqwest::Client, hash: String, total: &Arc<AtomicU64>, i: &Arc<AtomicU64>) -> eyre::Result<()> {
  let r = client.get(&format!("http://cache.nixos.org/{}.narinfo", hash)).send().await?;
  if r.status() != 200 {
    return Err(eyre::eyre!("bad status: {}", r.status()));
  }
  let r = r.text().await?; // maybe https is faster
  let d = r.lines().filter_map(|l| {
    l.find(": ").map(|i| (&l[..i], &l[i+2..]))
  }).collect::<HashMap<_,_>>();
  //dbg!(&d);
  let size: u64 = d.get("FileSize").ok_or(eyre::eyre!("no file size"))?.parse()?;
  total.fetch_add(size, Ordering::SeqCst);
  let total = total.load(Ordering::SeqCst);
  let i = i.load(Ordering::SeqCst);
  println!("{}: {} / {} = {}", hash, total, i, total / i);
  Ok(())
}

async fn process_hash(client: &reqwest::Client, hash: String, total: &Arc<AtomicU64>, i: &Arc<AtomicU64>) -> eyre::Result<()> {
  println!("processing {}", hash);
  let r = client.get(&format!("http://cache.nixos.org/{}.narinfo", hash)).send().await?;
  if r.status() != 200 {
    return Err(eyre::eyre!("bad status: {}", r.status()));
  }
  let r = r.text().await?; // maybe https is faster
  println!("{}", r);
  let d = r.lines().filter_map(|l| {
    l.find(": ").map(|i| (&l[..i], &l[i+2..]))
  }).collect::<HashMap<_,_>>();
  //dbg!(&d);
  let size: u64 = d.get("FileSize").ok_or(eyre::eyre!("no file size"))?.parse()?;
  total.fetch_add(size, Ordering::SeqCst);
  let total = total.load(Ordering::SeqCst);
  let i = i.load(Ordering::SeqCst);
  println!("{}: {} / {} = {}", hash, total, i, total / i);

  let url = d.get("URL").ok_or(eyre::eyre!("no url"))?;
  let r = client.get(format!("http://cache.nixos.org/{}", url)).send().await?;
  if r.status() != 200 {
    return Err(eyre::eyre!("bad status: {}", r.status()));
  }

  // convert a Steamer of Bytes to an AsyncReader
  let bs = r.bytes_stream();
  let ms = bs.map(|result| result.map_err(|err| {
    std::io::Error::new(std::io::ErrorKind::Other, err)
  }));
  let sr = StreamReader::new(ms);


  let mut hasher = AsyncSha256Hasher::new();
  tokio::io::copy(
    &mut async_compression::tokio::bufread::XzDecoder::new(sr),
    &mut hasher
  ).await?;
  let narhash = hasher.finalize();
  println!("computed hash: {}", hex::encode(narhash));
  println!("computed hash in base32: {}", nix_base32::to_nix_base32(&narhash));

  Ok(())
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;
  let db_addr = std::env::var("DB_ADDR").unwrap_or("10.42.0.7".to_string());

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&format!("postgresql://postgres@{db_addr}/nar-dedup")).await?;

  tracing::info!("connected to db");
  
  // setting up workers
  let total = Arc::new(AtomicU64::new(0));
  let i = Arc::new(AtomicU64::new(0));

  let (send, recv) = async_channel::bounded(1000);
  let tasks = (0..1).map(|thread_id| {
    tracing::info!(thread_id, "starting insertion");

    let recv = recv.clone();
    let pool = pool.clone();
    let total = total.clone();
    let i = i.clone();

    tokio::task::spawn(async move {
      let client = reqwest::Client::new();

      while let Ok(hash) = recv.recv().await {
        i.fetch_add(1, Ordering::SeqCst);

        let r = process_hash(&client, hash, &total, &i).await;

        if let Err(e) = r {
          tracing::error!(thread_id, ?e);
        } 
      }
    })
  }).collect::<Vec<_>>();

  let mut res = sqlx::query(r#"select * from store_hashes"#)
    .fetch(&pool);

  while let Some(row) = res.try_next().await? {
    let r: eyre::Result<()> = async {
      
      let s = row.columns().into_iter().filter_map(|c|{
        if let Some(Oid(1043)) = c.type_info().oid() {
          Some(format!("{}: {}", c.name(), row.try_get::<&str, _>(c.ordinal()).unwrap_or("<NULL>")))
        } else {
          None
        }
      }).collect::<Vec<_>>().join(", ");
      //println!("{}", s);

      let hash = row.try_get::<&str, _>("store_hash")?;
      send.send(hash.to_owned()).await?;

      Ok(())
    }.await;

    if let Err(e) = r {
      tracing::error!(?e);
    } 

    break;

  }
  drop(send);
  for h in tasks {
    h.await?;
  }



  Ok(())
}
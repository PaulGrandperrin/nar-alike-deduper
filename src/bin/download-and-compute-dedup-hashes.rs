use async_channel::Receiver;
use color_eyre::eyre;
use sqlx::{postgres::{PgPoolOptions, PgTypeInfo, types::Oid}, Row, Column};
use futures::{TryStreamExt, StreamExt};
use std::{fmt::Write, collections::HashMap, sync::{atomic::{AtomicU64, Ordering}, Arc}};

#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;
  let db_addr = std::env::var("DB_ADDR").unwrap_or("10.42.0.7".to_string());

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect(&format!("postgresql://postgres@{db_addr}/nar-dedup")).await?;

  tracing::info!("connected to db");
  
  // setting up workers
  let mut total = Arc::new(AtomicU64::new(0));
  let mut i = Arc::new(AtomicU64::new(0));

  let (send, recv) = async_channel::bounded(1000);
  let tasks = (0..16).map(|thread_id| {
    tracing::info!(thread_id, "starting insertion");

    let recv = recv.clone();
    let pool = pool.clone();
    let total = total.clone();
    let i = i.clone();

    tokio::task::spawn(async move {
      let c = reqwest::Client::new();

      while let Ok(hash) = recv.recv().await {
        i.fetch_add(1, Ordering::SeqCst);
        let r: eyre::Result<()> = async {
          return Ok(());
          let r = c.get(&format!("http://cache.nixos.org/{}.narinfo", hash)).send().await?;
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
        }.await;

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

  }
  drop(send);
  for h in tasks {
    h.await?;
  }



  Ok(())
}
use chrono::{DateTime, Utc};
use color_eyre::eyre::{self, OptionExt};
use duct::cmd;
use regex::Regex;
use reqwest::header::USER_AGENT;
use sqlx::postgres::PgPoolOptions;


async fn get_latest_revision(branch: &str) -> eyre::Result<String> {
  let c = reqwest::Client::new();
  let r = c.get(format!("https://api.github.com/repos/NixOS/nixpkgs/branches/{branch}"))
    .header(USER_AGENT, "nar-dedup")
    .send().await?;

  let j = r.json::<serde_json::Value>().await?;
  let revision = j.as_object().ok_or_eyre("bad json")?
    .get("commit").ok_or_eyre("bad json")?.as_object().ok_or_eyre("bad json")?
    .get("sha").ok_or_eyre("bad json")?.as_str().ok_or_eyre("bad json")?;

  Ok(revision.to_string())
}

async fn get_commit_date(revision: &str) -> eyre::Result<DateTime<Utc>> {
  let c = reqwest::Client::new();
  let r = c.get(format!("https://api.github.com/repos/NixOS/nixpkgs/commits/{revision}"))
    .header(USER_AGENT, "nar-dedup")
    .send().await?;

  let j = r.json::<serde_json::Value>().await?;
  let date = j.as_object().ok_or_eyre("bad json")?
    .get("commit").ok_or_eyre("bad json")?.as_object().ok_or_eyre("bad json")?
    .get("committer").ok_or_eyre("bad json")?.as_object().ok_or_eyre("bad json")?
    .get("date").ok_or_eyre("bad json")?.as_str().ok_or_eyre("bad json")?;

 
  Ok(date.parse::<DateTime<Utc>>()?)
}

#[tracing::instrument(fields(revision))]
async fn update(branch: &str, system: &str) -> eyre::Result<()> {
  tracing::info!("start");

  let regex = Regex::new(r"^(.*?)-([^a-zA-Z].*)$")?; // mimics https://github.com/NixOS/nix/blob/0fb5024d8df46a47f5367c5b0a51f0b2f6d50032/src/libstore/names.cc#L30

  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgresql://postgres@10.42.0.7/nar-dedup").await?;

  tracing::info!("connected to db");

  sqlx::query(r#"create table if not exists "store_hashes" (
    branch varchar(100) not null,
    revision char(40) not null,
    system varchar(100) not null,
    commit_date timestamptz not null,
    path varchar(1000) not null,
    name varchar(1000) not null,
    pname varchar(1000),
    version varchar(100),
    store_hash char(32) not null,
    processed_by varchar(100),
    processed_since timestamptz
  )"#).execute(&pool).await?;

  sqlx::query(r#"create table if not exists "completed_drv_sets" (
    branch varchar(100) not null,
    revision char(40) not null,
    system varchar(100) not null
  )"#).execute(&pool).await?;

  tracing::info!("created tables");

  let revision = get_latest_revision(branch).await?;

  tracing::Span::current().record("revision", &revision.as_str());

  let res = sqlx::query("select * from completed_drv_sets where branch = $1 and revision = $2 and system = $3")
    .bind(branch)
    .bind(revision.clone())
    .bind(system)
    .fetch_all(&pool).await?;

  if res.len() > 0 {
    tracing::info!("already in db");
    return Ok(());
  }

  tracing::info!("processing");

  let out = cmd!("nix", "eval", "--json", "--file", "nixpkgs-hashes.nix")
    .env("FLAKE_URL", format!("github:NixOS/nixpkgs/{}", revision))
    .env("NIX_SYSTEM", system)
    .read()?;
  let json = serde_json::from_str::<serde_json::Value>(&out)?;

  tracing::info!("processing successful");
  tracing::info!("inserting in db");

  let commit_date = get_commit_date(&revision).await?;

  for drv in json.as_array().ok_or_eyre("bad json")? {
    let drv = drv.as_object().ok_or_eyre("bad json")?;
    let path = drv.get("name").ok_or_eyre("bad json")?
      .as_array().ok_or_eyre("bad json")?
      .iter().map(|v| v.as_str().unwrap_or_default()).collect::<Vec<_>>().join(".");
    let store_paths = drv.get("value").ok_or_eyre("bad json")?
      .as_array().ok_or_eyre("bad json")?
      .iter().map(|v| v.as_str().unwrap_or_default()).collect::<Vec<_>>();

    for sp in store_paths {
      let sh = sp[11..43].to_string();
      let name = sp[44..].to_string();

      let (pname, version) = match regex.captures(&name) {
        Some(c) => (c.get(1).map(|e| e.as_str()), c.get(2).map(|e| e.as_str())),
        None => (None, None)
      }; 

      sqlx::query("insert into store_hashes
        (branch, revision, system, commit_date, path, name, pname, version, store_hash)
        values ($1, $2, $3, $4, $5, $6, $7, $8, $9)")
        .bind(branch)
        .bind(revision.clone())
        .bind(system)
        .bind(commit_date)
        .bind(path.clone()) 
        .bind(name.clone()) 
        .bind(pname)
        .bind(version)
        .bind(sh)
        .execute(&pool).await?;
    }
  }

  sqlx::query("insert into completed_drv_sets
    (branch, revision, system)
    values ($1, $2, $3)")
    .bind(branch)
    .bind(revision)
    .bind(system)
    .execute(&pool).await?;

  tracing::info!("insert in db successful");

  Ok(())

}

async fn delete() -> eyre::Result<()> {
  let pool = PgPoolOptions::new()
    .max_connections(5)
    .connect("postgresql://postgres@10.42.0.7/nar-dedup").await?;

  sqlx::query(r#"drop table if exists "store_hashes""#).execute(&pool).await?;
  sqlx::query(r#"drop table if exists "completed_drv_sets""#).execute(&pool).await?;

  Ok(())

}

#[tokio::main]
async fn main() -> eyre::Result<()> {
  nar_alike_deduper::setup_logging()?;

  loop {
    update("nixos-23.11", "x86_64-linux").await?;
    tracing::info!("sleeping 60 sec");
    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
  }

}
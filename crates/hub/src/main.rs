//! `hrm-hub`: the hub from the environment. `HUB_ADMIN_TOKEN` (needed), `HUB_DATA` (/data), `HUB_BIND`
//! (0.0.0.0:8787), `HUB_MAX_BLOB_MB` (64), `HUB_RETAIN_MB` (2048).

fn env_or(name: &str, default: &str) -> String { std::env::var(name).ok().filter(|s| !s.is_empty()).unwrap_or_else(|| default.into()) }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = hrm_hub::Config {
        data_dir: env_or("HUB_DATA", "/data").into(),
        admin_token: env_or("HUB_ADMIN_TOKEN", ""),
        bind: env_or("HUB_BIND", "0.0.0.0:8787").parse()?,
        max_blob_mb: env_or("HUB_MAX_BLOB_MB", "64").parse()?,
        retain_mb: env_or("HUB_RETAIN_MB", "2048").parse()?,
    };
    let (addr, _handle) = hrm_hub::serve(cfg).await?;
    eprintln!("hrm-hub listening on {addr}");
    std::future::pending::<()>().await;
    Ok(())
}

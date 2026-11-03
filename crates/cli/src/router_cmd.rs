use std::sync::Arc;

use anyhow::Result;
use clap::Subcommand;
use hrm_core::Ctx;

#[derive(Subcommand)]
pub enum RouterCmd {
    /// Run the router in this console (the app and `hrm run` start it in the background when needed)
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
    /// Is the router running, and on which port
    Status,
    /// Stop the background router
    Stop,
    /// Requests the router handled, newest first
    Log {
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    /// Models a route offers (aggregate routes prefix each with its provider)
    Models { route: String },
}

pub fn run(ctx: &Ctx, cmd: RouterCmd) -> Result<i32> {
    match cmd {
        RouterCmd::Serve { port } => {
            let port = port.unwrap_or(ctx.settings.router_port);
            hrm_core::service::router_token(ctx)?;
            let ctx = Arc::new(Ctx::open()?);
            println!("router listening on 127.0.0.1:{port}");
            tokio::runtime::Runtime::new()?.block_on(hrm_router::serve(ctx, port))?;
            Ok(0)
        }
        RouterCmd::Status => {
            let up = hrm_core::service::router_running(ctx);
            println!("router {} on 127.0.0.1:{}", if up { "running" } else { "stopped" }, ctx.settings.router_port);
            Ok(0)
        }
        RouterCmd::Stop => {
            println!("{}", if hrm_core::service::router_stop(ctx)? { "router stopped" } else { "router was not running" });
            Ok(0)
        }
        RouterCmd::Log { limit } => {
            for r in ctx.db.route_log(limit)? {
                println!("{:<13} {:<12} {:<16} {:<12} {:<28} {:>4} {:>6}ms {:>7}/{:<7}", r["ts"], r["route"].as_str().unwrap_or(""), r["profile"].as_str().unwrap_or(""),
                    r["provider"].as_str().unwrap_or(""), r["model"].as_str().unwrap_or(""), r["status"].as_i64().unwrap_or(0), r["ms"].as_i64().unwrap_or(0),
                    r["input"].as_i64().unwrap_or(0), r["output"].as_i64().unwrap_or(0));
            }
            Ok(0)
        }
        RouterCmd::Models { route } => {
            let reg = ctx.registry()?;
            let r = reg.routes.iter().find(|r| r.id == route).ok_or_else(|| anyhow::anyhow!("no route {route}"))?;
            for m in hrm_router::model_list(&reg, r) { println!("{}", m["id"].as_str().unwrap_or("")); }
            Ok(0)
        }
    }
}

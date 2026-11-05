//! Where the time goes on this computer's real data: `cargo run -p hrm-core --example timing`.
//! Reads the real data folder (HRM_HOME or the default); the import writes cursors as the app would.

use std::time::Instant;

fn main() -> anyhow::Result<()> {
    let ctx = hrm_core::Ctx::open()?;
    let reg = ctx.registry()?;
    println!("profiles: {}", reg.profiles.len());
    for p in &reg.profiles {
        let h = hrm_core::harness::of(p);
        let t = Instant::now();
        let n = h.import_usage(&ctx, p).map(|n| n.to_string()).unwrap_or_else(|e| format!("error {e:#}"));
        println!("import {:<24} {:<10} {:>7} ms  rows {}", p.name, h.id(), t.elapsed().as_millis(), n);
    }
    let t = Instant::now();
    let since = hrm_core::util::now_ms() - 30 * 86_400_000;
    for p in &reg.profiles {
        let h = hrm_core::harness::of(p);
        let t = Instant::now();
        let n = h.sessions(&ctx, p, since).map(|v| v.len()).unwrap_or(0);
        println!("sessions {:<22} {:<10} {:>7} ms  {} sessions", p.name, h.id(), t.elapsed().as_millis(), n);
    }
    println!("all harness sessions: {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let r = hrm_core::index::refresh(&ctx, hrm_core::index::Scope::All)?;
    println!("index::refresh(All): {} ms, {} sessions, {} written, {} removed", t.elapsed().as_millis(), r.sessions, r.changed, r.removed);
    let t = Instant::now();
    let f = hrm_core::service::SessionFilter::default();
    let v = hrm_core::service::sessions(&ctx, &f)?;
    println!("service::sessions (journaling, projects, tags): {} ms, {} rows", t.elapsed().as_millis(), v.len());
    let page = hrm_core::service::SessionFilter { per: Some(50), page: Some(0), days: Some(7), ..Default::default() };
    let t = Instant::now();
    let v = hrm_core::service::sessions(&ctx, &page)?;
    println!("service::sessions (a page of 50, 7 days): {} ms, {} rows", t.elapsed().as_millis(), v.len());
    let t = Instant::now();
    let n = hrm_core::service::sessions_total(&ctx, &page)?;
    println!("sessions_total: {} ms, {} rows", t.elapsed().as_millis(), n);
    let t = Instant::now();
    let g = hrm_core::service::grouped_sessions(&ctx, &page, "project")?;
    println!("grouped_sessions (the Sessions page: a page of 50, 7 days): {} ms, {} groups", t.elapsed().as_millis(), g.len());
    let t = Instant::now();
    let g = hrm_core::service::grouped_sessions(&ctx, &hrm_core::service::SessionFilter { query: Some("build".into()), ..page.clone() }, "project")?;
    println!("grouped_sessions with a search: {} ms, {} groups", t.elapsed().as_millis(), g.len());
    let t = Instant::now();
    let g = hrm_core::service::grouped_sessions(&ctx, &f, "project")?;
    println!("grouped_sessions: {} ms, {} groups", t.elapsed().as_millis(), g.len());
    let t = Instant::now();
    let pr = hrm_core::service::projects(&ctx, None, None)?;
    println!("projects: {} ms, {} projects", t.elapsed().as_millis(), pr.len());
    let t = Instant::now();
    let _ = hrm_core::service::overview(&ctx, Some(7), None, None)?;
    println!("overview query: {} ms", t.elapsed().as_millis());
    let t = Instant::now();
    let _ = hrm_core::dash::dashboard(&ctx, 0, None, None)?;
    println!("dashboard today: {} ms", t.elapsed().as_millis());
    Ok(())
}

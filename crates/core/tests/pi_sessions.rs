//! Pi and Oh My Pi session logs (fixtures under tests/fixtures/pi and omp): usage rows, session facts, the
//! transcript, and an import that counts nothing twice.

use std::path::Path;

use hrm_core::harness::{self, pi};
use hrm_core::registry::{Kind, Profile};
use hrm_core::Ctx;

fn fixtures() -> std::path::PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures") }

fn copy_tree(from: &Path, to: &Path) {
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let t = to.join(e.file_name());
        if e.path().is_dir() { copy_tree(&e.path(), &t); } else { std::fs::create_dir_all(to).unwrap(); std::fs::copy(e.path(), t).unwrap(); }
    }
}

/// A sandbox whose home holds the fixtures as the two tools would keep them, with a main profile for each.
fn ctx() -> (tempfile::TempDir, Ctx, Profile, Profile) {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    copy_tree(&fixtures().join("pi"), &home.join(".pi").join("agent").join("sessions"));
    copy_tree(&fixtures().join("omp"), &home.join(".omp").join("agent").join("sessions"));
    let ctx = Ctx::open_at(&dir.path().join("data"), &home).unwrap();
    let p = |name: &str, h: &str| Profile { name: name.into(), label: None, harness: h.into(), kind: Kind::Subscription, email: None, main: true, base_url: None, model: None, route: None, daily_cap_usd: None, created_at: 0 };
    (dir, ctx, p("pi-main", "pi"), p("omp-main", "omp"))
}

fn read(rel: &str) -> String { std::fs::read_to_string(fixtures().join(rel)).unwrap() }

#[test]
fn pi_usage_rows_count_every_model_call_with_cache_and_timing() {
    let rows = pi::usage_rows(&read("pi/--D--Projects-demo--/2026-11-03T09-12-00-000Z_0193a7e2-5b1c-7f00-8c5e-1a2b3c4d5e6f.jsonl"), "pi", "pi-main");
    assert_eq!(rows.len(), 5, "four replies and one cache-warm usage entry");
    assert_eq!(rows.iter().map(|r| r.input).sum::<i64>(), 5710);
    assert_eq!(rows.iter().map(|r| r.output).sum::<i64>(), 215);
    assert_eq!(rows.iter().map(|r| r.cache_read).sum::<i64>(), 1500);
    assert_eq!(rows.iter().map(|r| r.cache_write).sum::<i64>(), 1650);
    assert_eq!(rows[0].request_id, "pi:0193a7e2-5b1c-7f00-8c5e-1a2b3c4d5e6f:c3d4e5f6");
    assert_eq!((rows[0].project.as_str(), rows[0].model.as_str(), rows[0].session_id.as_str()), ("D:\\Projects\\demo", "claude-sonnet-5-5", "0193a7e2-5b1c-7f00-8c5e-1a2b3c4d5e6f"));
    assert_eq!(rows[0].ts, 1793697126500, "a reply is dated when it began");
    assert_eq!(rows[0].gen_ms, Some(2500), "from the reply's start to the entry being written");
    assert_eq!(rows[4].model, "claude-sonnet-5-5");
    assert!(rows.iter().all(|r| r.ttft_ms.is_none()), "no first-token time in the log");
}

#[test]
fn oh_my_pi_rows_skip_the_title_slot_and_count_model_usage() {
    let rows = pi::usage_rows(&read("omp/-Projects-demo/2026-11-04T10-00-00-000Z_0193b1c0-7a1e-7c3d-9e2f-0a1b2c3d4e5f.jsonl"), "omp", "omp-main");
    assert_eq!(rows.len(), 3, "two replies and the title call");
    assert_eq!(rows.iter().map(|r| r.input).sum::<i64>(), 2400);
    assert_eq!(rows.iter().map(|r| r.output).sum::<i64>(), 112);
    let title = rows.iter().find(|r| r.request_id.ends_with(":0d0d0d0d")).unwrap();
    assert_eq!((title.model.as_str(), title.input, title.ts), ("deepseek-flash", 500, 1793786430000));
    assert!(rows.iter().all(|r| r.harness == "omp" && r.project == "C:\\Users\\alex\\Projects\\demo"));
}

#[test]
fn session_facts_and_the_transcript() {
    let (_dir, ctx, pi_main, omp_main) = ctx();
    let h = harness::by_id("pi").unwrap();
    let mut s = h.sessions(&ctx, &pi_main, 0).unwrap();
    s.sort_by_key(|m| m.started_at);
    assert_eq!(s.len(), 2);
    let m = &s[0];
    assert_eq!(m.title, "Add a retry to the upload script");
    assert_eq!((m.cwd.as_str(), m.prompts, m.tool_calls, m.failed, m.messages), ("D:\\Projects\\demo", 1, 3, 1, 5));
    assert_eq!((m.tokens, m.model.as_str()), (5915, "claude-sonnet-5-5"));
    assert_eq!((m.started_at, m.last_active_at), (1793697120000, 1793697180000));
    assert_eq!(s[1].title, "Retry upload (fork)", "a forked session is named by its session_info");
    assert_eq!(s[1].id, "0193a7f0-2222-7f00-8c5e-1a2b3c4d5e6f");
    assert_eq!(h.resume_args(&m.id), vec!["--session", m.id.as_str()]);

    let t = h.transcript(&ctx, &pi_main, m).unwrap();
    assert_eq!(t.turns.len(), 1);
    let turn = &t.turns[0];
    assert_eq!(turn.prompt, "Add a retry to the upload script");
    let kinds: Vec<(&str, &str, &str)> = turn.steps.iter().map(|s| (s.kind.as_str(), s.tool.as_str(), s.status.as_str())).collect();
    assert_eq!(kinds, vec![("thinking", "", ""), ("tool", "read", "ok"), ("tool", "edit", "ok"), ("tool", "bash", "error")]);
    assert_eq!(turn.steps[1].input, "scripts/upload.sh");
    assert_eq!(turn.steps[1].duration_ms, Some(200));
    let c = turn.steps[2].change.as_ref().unwrap();
    assert_eq!((c.path.as_str(), c.added, c.removed), ("scripts/upload.sh", 1, 1));
    assert!(turn.steps[3].error.contains("Could not resolve host"));
    assert_eq!(turn.answer, vec!["Let me read it.", "The upload now retries three times; the host could not be resolved from here, so test it on the build machine."]);
    assert_eq!(turn.model, "claude-sonnet-5-5");

    let o = harness::by_id("omp").unwrap();
    let s = o.sessions(&ctx, &omp_main, 0).unwrap();
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].title, "Fix the flaky build", "the title slot names the session");
    assert_eq!((s[0].model.as_str(), s[0].tokens, s[0].prompts, s[0].tool_calls), ("deepseek-v4-pro", 2000, 1, 1));
    assert_eq!(o.resume_args(&s[0].id), vec!["--resume", s[0].id.as_str()]);
    let t = o.transcript(&ctx, &omp_main, &s[0]).unwrap();
    assert_eq!(t.turns[0].steps[1].result, "test result: ok. 212 passed");
}

#[test]
fn importing_twice_stores_every_row_once() {
    let (_dir, ctx, pi_main, omp_main) = ctx();
    let (h, o) = (harness::by_id("pi").unwrap(), harness::by_id("omp").unwrap());
    assert_eq!(h.import_usage(&ctx, &pi_main).unwrap(), 7, "five rows in the first session, two in the fork");
    assert_eq!(o.import_usage(&ctx, &omp_main).unwrap(), 3);
    assert_eq!(h.import_usage(&ctx, &pi_main).unwrap() + o.import_usage(&ctx, &omp_main).unwrap(), 0, "unchanged files are skipped");
    // the files read again from scratch (a lost cursor): the same ids, so nothing is added
    ctx.db.with(|c| { c.execute("delete from cursors", [])?; Ok(()) }).unwrap();
    h.import_usage(&ctx, &pi_main).unwrap();
    o.import_usage(&ctx, &omp_main).unwrap();
    let n: i64 = ctx.db.with(|c| Ok(c.query_row("select count(*) from usage", [], |r| r.get(0))?)).unwrap();
    assert_eq!(n, 10);
    let by: Vec<(String, i64)> = ctx.db.with(|c| {
        let mut st = c.prepare("select harness, sum(input + output) from usage group by harness order by harness")?;
        let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).unwrap();
    assert_eq!(by, vec![("omp".to_string(), 2512), ("pi".to_string(), 10065)]);
}

#[test]
fn both_tools_are_registered_with_their_homes_and_caps() {
    let (_dir, ctx, pi_main, omp_main) = ctx();
    let (h, o) = (harness::by_id("pi").unwrap(), harness::by_id("omp").unwrap());
    assert!(h.home(&ctx, &pi_main).ends_with(Path::new(".pi").join("agent")));
    assert!(o.home(&ctx, &omp_main).ends_with(Path::new(".omp").join("agent")));
    let work = Profile { name: "work".into(), main: false, ..pi_main.clone() };
    assert!(h.home(&ctx, &work).ends_with(".pi-work"));
    assert!(h.caps().api_key && h.caps().routable && !o.caps().api_key && !o.caps().routable);
    assert!(!h.caps().quota && !o.caps().quota);
    let ip = h.item_paths(&ctx, &pi_main);
    assert!(ip.mcp.unwrap().0.ends_with("mcp.json") && ip.skills.unwrap().ends_with("skills") && ip.prompt.unwrap().ends_with("AGENTS.md"));
    assert!(!h.identity(&ctx, &pi_main).logged_in, "no auth.json yet");
    std::fs::write(h.home(&ctx, &pi_main).join("auth.json"), r#"{"anthropic":{"type":"oauth"},"deepseek":{"type":"api_key","key":"x"}}"#).unwrap();
    assert_eq!(h.identity(&ctx, &pi_main).plan.as_deref(), Some("anthropic, deepseek"));
}

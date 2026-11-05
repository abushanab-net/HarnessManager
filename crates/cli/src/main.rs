//! `hrm`: Harness Manager from the command line. Every command has `--json` for scripts.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use hrm_core::registry::{Kind, McpServer, Provider, Route};
use hrm_core::service::{self, AddRequest, SessionFilter};
use hrm_core::{harness, items, Ctx};

mod router_cmd;

#[derive(Parser)]
#[command(name = "hrm", version, about = "Run Claude Code, Codex, OpenCode and Antigravity under several accounts side by side")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Args, Clone, Copy)]
struct Json {
    /// Print JSON instead of a table
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand)]
enum Cmd {
    /// List accounts with plan, limits, tokens and open consoles
    List { #[command(flatten)] j: Json, /// Import new usage first
        #[arg(long)] refresh: bool },
    /// Show one account in detail
    Show { name: String, #[command(flatten)] j: Json },
    /// Add an account (subscription login by default)
    Add {
        name: String,
        #[arg(long, default_value = "claude")] harness: String,
        /// Use an API key instead of a subscription login (prompted, or from HRM_API_KEY)
        #[arg(long)] api_key: bool,
        /// Send this profile through the local router using a route
        #[arg(long)] route: Option<String>,
        #[arg(long)] email: Option<String>,
        #[arg(long)] base_url: Option<String>,
        #[arg(long)] model: Option<String>,
        /// Create without starting the login
        #[arg(long)] no_login: bool,
        #[command(flatten)] j: Json,
    },
    /// Log an account in (interactive)
    Login { name: String },
    /// Remove an account; --delete-home also deletes its folder
    Remove { name: String, #[arg(long)] delete_home: bool },
    /// Run the harness as an account in this console; arguments after -- go to the harness
    Run { name: String, #[arg(last = true)] args: Vec<String> },
    /// Open a new terminal running the account
    Start { name: String, #[arg(long)] cwd: Option<PathBuf>, #[arg(last = true)] args: Vec<String> },
    /// API keys for API-key accounts (kept in Windows Credential Manager)
    Key { #[command(subcommand)] cmd: KeyCmd },
    /// Token usage per account and day
    Usage { #[arg(long, default_value_t = 7)] days: i64, #[arg(long)] profile: Option<String>, #[command(flatten)] j: Json },
    /// Totals: cost at API list prices, requests, tokens and cache hit rate
    Overview { #[arg(long)] days: Option<i64>, #[arg(long)] harness: Option<String>, #[command(flatten)] j: Json },
    /// Every model response, newest first
    Requests { #[arg(long)] days: Option<i64>, #[arg(long)] harness: Option<String>, #[arg(long, default_value_t = 30)] limit: i64, #[command(flatten)] j: Json },
    /// Limits of one account; --live asks Anthropic now (Claude subscription accounts)
    Quota { name: String, #[arg(long)] live: bool, #[command(flatten)] j: Json },
    /// Sessions of every tool, grouped by project (or date, account, tool)
    Sessions {
        #[arg(long, default_value = "project")] group: String,
        #[arg(long)] harness: Option<String>,
        #[arg(long)] profile: Option<String>,
        #[arg(long)] days: Option<i64>,
        #[arg(long)] search: Option<String>,
        /// Only sessions a console is working in now
        #[arg(long)] open: bool,
        /// Only one project, by its key
        #[arg(long)] project: Option<String>,
        /// Only sessions carrying this tag (their own or their project's)
        #[arg(long)] tag: Option<String>,
        #[command(flatten)] j: Json,
    },
    /// Tag a session (session <profile>/<id>) or a project (project <key>): the tags given replace its tags; none clears them
    Tag { kind: String, key: String, tags: Vec<String> },
    /// Every tag, with how many sessions and projects carry it
    Tags { #[command(flatten)] j: Json },
    /// Print one session as a transcript
    Session { profile: String, id: String, #[command(flatten)] j: Json },
    /// Reopen a session in a new terminal, in its own account
    Resume { profile: String, id: String, #[arg(long)] cwd: Option<PathBuf> },
    /// Projects: list, rename or merge
    Projects { #[command(subcommand)] cmd: ProjectCmd },
    /// Bring every account in step: shared Claude settings, MCP servers and skills
    Sync { /// Replace folders the old PowerShell launcher copied with links to the main profile
        #[arg(long)] relink: bool, #[command(flatten)] j: Json },
    /// MCP servers shared across tools
    Mcp { #[command(subcommand)] cmd: McpCmd },
    /// Skills shared across tools
    Skills { #[command(subcommand)] cmd: SkillCmd },
    /// Prompt files (CLAUDE.md, AGENTS.md) per tool
    Prompts { #[command(subcommand)] cmd: PromptCmd },
    /// Router providers (endpoints and keys)
    Provider { #[command(subcommand)] cmd: ProviderCmd },
    /// Routes: failover or aggregate across providers
    Route { #[command(subcommand)] cmd: RouteCmd },
    /// The local router
    Router { #[command(subcommand)] cmd: router_cmd::RouterCmd },
    /// Import accounts from the old claude-accounts.ps1 launcher
    Import { path: Option<PathBuf>, #[command(flatten)] j: Json },
    /// Detect installed tools and add their main profiles
    Init { #[command(flatten)] j: Json },
    /// Supported tools and what each can do
    Harnesses { #[command(flatten)] j: Json },
    /// Read or change settings
    Settings { key: Option<String>, value: Option<String> },
    /// Across computers: the shared folder, this computer and the others
    Computers { #[command(subcommand)] cmd: ComputersCmd },
    /// Open the desktop app
    Open,
    /// Resume a session from another computer here, as a fork: <computer>/<tool>/<account>/<session id>
    ResumeHere { session: String },
    /// Transcripts carried to the folder and known from the other computers
    Transcripts { #[command(flatten)] j: Json },
    /// Make a backup now (the account list, settings, the history; never keys or logins)
    Backup {
        /// Also this computer's session transcripts, compressed
        #[arg(long)] with_transcripts: bool,
        #[command(flatten)] j: Json,
    },
    /// Merge the history in a backup zip or a sync folder into this computer; nothing is counted twice
    Restore {
        /// A backup zip, or the sync folder
        source: PathBuf,
        /// Continue as the computer named in it: its id, a new incarnation
        #[arg(long)] adopt: bool,
        /// Only say what it holds
        #[arg(long)] preview: bool,
        #[command(flatten)] j: Json,
    },
    /// Old transcripts out of the tools' folders into the archive; the sessions stay listed and readable
    Archive { #[command(subcommand)] cmd: ArchiveCmd },
}

#[derive(Subcommand)]
enum ArchiveCmd {
    /// How many sessions are archived, where, and when the step last ran
    Status { #[command(flatten)] j: Json },
    /// Run the step now with the archive_days setting
    Now { #[command(flatten)] j: Json },
}

#[derive(Subcommand)]
enum ComputersCmd {
    /// This computer, the last round and the other computers
    Status { #[command(flatten)] j: Json },
    /// Publish what is new here and merge what the others published, now
    Now { #[command(flatten)] j: Json },
    /// Change the name, the mode (off or folder), the folder, and what is shared
    Set {
        #[arg(long)] name: Option<String>,
        #[arg(long)] mode: Option<String>,
        #[arg(long)] folder: Option<PathBuf>,
        /// on or off
        #[arg(long)] transcripts: Option<String>,
        /// on or off
        #[arg(long)] presence: Option<String>,
        /// on or off: talk to paired computers straight over the network (mDNS or an address)
        #[arg(long)] direct: Option<String>,
        /// The direct connection's port; 0 picks a free one once
        #[arg(long)] port: Option<u16>,
        #[command(flatten)] j: Json,
    },
    /// Show a six-digit code and wait up to two minutes for another computer to join with it
    Pair {
        /// Print the host form (name:port) for `join --host`; a value replaces this computer's name in it (a VPN name, an IP)
        #[arg(long, num_args = 0..=1, default_missing_value = "")] host: Option<String>,
    },
    /// Pair with the computer showing a code: found by mDNS, or at --host name:port
    Join {
        code: String,
        #[arg(long)] host: Option<String>,
    },
    /// Forget a paired computer (what was merged from it stays)
    Forget { id: String },
    /// The paired and known computers, how each is reached
    Peers { #[command(flatten)] j: Json },
    /// Run the direct connection's listener (and mDNS) for n seconds without the app
    Serve { #[arg(long)] seconds: u64 },
}

#[derive(Subcommand)]
enum KeyCmd {
    /// Save or replace the key (prompted, or from HRM_API_KEY)
    Set { name: String },
    /// Delete the saved key
    Delete { name: String },
}

#[derive(Subcommand)]
enum ProjectCmd {
    List { /// Only the last N days (default: all time)
        #[arg(long)] days: Option<i64>, #[command(flatten)] j: Json },
    Rename { key: String, name: String },
    Merge { key: String, into: String },
    Reset { key: String },
}

#[derive(Subcommand)]
enum McpCmd {
    List { #[command(flatten)] j: Json },
    /// Add a stdio server (--command) or an http/sse server (--url)
    Add {
        name: String,
        #[arg(long)] command: Option<String>,
        #[arg(long, num_args = 0.., allow_hyphen_values = true)] args: Vec<String>,
        #[arg(long)] url: Option<String>,
        #[arg(long, default_value = "")] transport: String,
        /// KEY=VALUE, repeatable
        #[arg(long = "env")] env: Vec<String>,
        /// Name: value, repeatable
        #[arg(long = "header")] headers: Vec<String>,
    },
    Remove { name: String },
    /// Enable for a tool (all its accounts) or one account with --profile
    Enable { name: String, harness: String, #[arg(long)] profile: Option<String> },
    Disable { name: String, harness: String, #[arg(long)] profile: Option<String> },
    /// Bring a tool's existing MCP servers under management
    Import { harness: String },
}

#[derive(Subcommand)]
enum SkillCmd {
    List { #[command(flatten)] j: Json },
    /// From a local folder with SKILL.md, or a GitHub URL (…/tree/<branch>/<path>)
    Install { source: String, #[arg(long)] name: Option<String> },
    Remove { name: String },
    Enable { name: String, harness: String, #[arg(long)] profile: Option<String> },
    Disable { name: String, harness: String, #[arg(long)] profile: Option<String> },
    Import { harness: String },
    /// Check installed skills for updates
    Check,
}

#[derive(Subcommand)]
enum PromptCmd {
    List { #[command(flatten)] j: Json },
    /// Add from a file
    Add { harness: String, title: String, #[arg(long)] file: PathBuf },
    Enable { id: String },
    Show { id: String },
}

#[derive(Subcommand)]
enum ProviderCmd {
    List { #[command(flatten)] j: Json },
    /// Add an endpoint; the key is prompted (or HRM_API_KEY)
    Add { id: String, #[arg(long)] base_url: String, #[arg(long, default_value = "anthropic")] format: String, #[arg(long, value_delimiter = ',')] models: Vec<String>, #[arg(long)] name: Option<String> },
    Remove { id: String },
}

#[derive(Subcommand)]
enum RouteCmd {
    List { #[command(flatten)] j: Json },
    /// --mode failover (try providers in order) or aggregate (one model list)
    Add { id: String, #[arg(long, default_value = "failover")] mode: String, #[arg(long, value_delimiter = ',')] providers: Vec<String>, #[arg(long)] default: Option<String> },
    Remove { id: String },
}

fn main() {
    // `hrm ... | head` closes the pipe early; that is not an error worth a panic message
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.to_string().contains("failed printing to stdout") { std::process::exit(0); }
        default_hook(info);
    }));
    let cli = Cli::parse();
    match real_main(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e:#}");
            std::process::exit(1);
        }
    }
}

fn print_json<T: serde::Serialize>(v: &T) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(v)?);
    Ok(())
}

fn fmt_tokens(n: i64) -> String {
    if n >= 1_000_000_000 { format!("{:.2}B", n as f64 / 1e9) } else if n >= 1_000_000 { format!("{:.2}M", n as f64 / 1e6) } else if n >= 1000 { format!("{:.1}k", n as f64 / 1e3) } else { n.to_string() }
}

fn fmt_reset(ms: Option<i64>) -> String {
    match ms.and_then(chrono::DateTime::from_timestamp_millis) {
        Some(d) => d.with_timezone(&chrono::Local).format("%a %H:%M").to_string(),
        None => "-".into(),
    }
}

fn secret_input(prompt: &str) -> Result<String> {
    if let Ok(k) = std::env::var("HRM_API_KEY") { return Ok(k); }
    let k = rpassword::prompt_password(prompt)?;
    if k.trim().is_empty() { bail!("no key entered"); }
    Ok(k)
}

fn parse_pairs(items: &[String], sep: char) -> Result<std::collections::BTreeMap<String, String>> {
    items.iter().map(|s| {
        let (k, v) = s.split_once(sep).with_context(|| format!("'{s}' should look like KEY{sep}VALUE"))?;
        Ok((k.trim().to_string(), v.trim().to_string()))
    }).collect()
}

fn real_main(cli: Cli) -> Result<i32> {
    let ctx = Ctx::open()?;
    // commands that may change the registry hold its lock; ones that wait on the user or run long do not
    // (`add` takes the lock inside add_profile, before its sign-in)
    let _reg_lock = (!matches!(cli.cmd, Cmd::Run { .. } | Cmd::Login { .. } | Cmd::Add { .. } | Cmd::Key { .. } | Cmd::Router { .. } | Cmd::Quota { .. } | Cmd::Start { .. } | Cmd::Resume { .. } | Cmd::Open | Cmd::Computers { .. } | Cmd::ResumeHere { .. }))
        .then(|| ctx.lock_registry());
    if !matches!(cli.cmd, Cmd::Init { .. } | Cmd::Run { .. }) {
        let _l = ctx.lock_registry();
        if let Err(e) = hrm_core::import::first_run(&ctx) { eprintln!("first-run setup: {e:#}"); }
    }
    match cli.cmd {
        Cmd::List { j, refresh } => {
            if refresh { service::import_usage(&ctx)?; }
            let views = service::list_views(&ctx)?;
            if j.json { return print_json(&views).map(|_| 0); }
            println!("{:<24} {:<12} {:<14} {:>8} {:>8} {:>9} {:>5}", "ACCOUNT", "TOOL", "PLAN", "5-HOUR", "WEEK", "TOKENS 7D", "OPEN");
            for v in views {
                let tier = |id: &str| v.quota.as_ref().and_then(|q| q.tier(id)).map(|t| format!("{:.0}%", t.used)).unwrap_or_else(|| "-".into());
                let plan = v.identity.plan.clone().unwrap_or_else(|| if v.identity.logged_in { "?".into() } else { "not logged in".into() });
                // a long plan (OpenCode lists every signed-in provider) is cut to its column
                println!("{:<24} {:<12} {:<14} {:>8} {:>8} {:>9} {:>5}", v.name, v.harness_label, hrm_core::util::truncate(&plan, 13), tier("session"), tier("week"), fmt_tokens(v.tokens_7d), v.running.len());
            }
        }
        Cmd::Show { name, j } => {
            let reg = ctx.registry()?;
            let v = service::profile_view(&ctx, reg.get(&name)?);
            if j.json { return print_json(&v).map(|_| 0); }
            println!("{} ({}, {})", v.name, v.harness_label, if v.main { "main profile" } else { "profile" });
            println!("  folder     {}", v.home);
            println!("  logged in  {}", if v.identity.logged_in { v.identity.email.clone().unwrap_or_else(|| "yes".into()) } else { "no".into() });
            if let Some(p) = &v.identity.plan { println!("  plan       {p}"); }
            if let Some(o) = &v.identity.org { println!("  org        {o}"); }
            if v.email_mismatch { println!("  warning    logged in as a different email than {}", v.expected_email.clone().unwrap_or_default()); }
            if let Some(q) = &v.quota {
                for t in &q.tiers { println!("  {:<10} {:>3.0}%  resets {}", t.label.chars().take(10).collect::<String>(), t.used, fmt_reset(t.resets_at)); }
            }
            println!("  tokens 7d  {}  ({} sessions)", fmt_tokens(v.tokens_7d), v.sessions_7d);
            for r in &v.running { println!("  open       pid {} {}", r.pid, r.cwd.clone().unwrap_or_default()); }
        }
        Cmd::Add { name, harness: h, api_key, route, email, base_url, model, no_login, j } => {
            let kind = if route.is_some() { Kind::Routed } else if api_key { Kind::ApiKey } else { Kind::Subscription };
            let key = if kind == Kind::ApiKey { Some(secret_input("API key: ")?) } else { None };
            let v = service::add_profile(&ctx, AddRequest { name: name.clone(), harness: h, kind, email, api_key: key, base_url, model, route, main: false })?;
            if j.json { print_json(&v)?; } else { println!("Added {} ({}) at {}", v.name, v.harness_label, v.home); }
            if kind == Kind::Subscription && !no_login {
                return login(&ctx, &name);
            }
        }
        Cmd::Login { name } => return login(&ctx, &name),
        Cmd::Remove { name, delete_home } => {
            service::remove_profile(&ctx, &name, delete_home)?;
            println!("Removed {name}{}", if delete_home { " and its folder" } else { "" });
        }
        Cmd::Run { name, args } => {
            let spec = service::launch_spec(&ctx, &name, &args)?;
            return spec.run();
        }
        Cmd::Start { name, cwd, args } => {
            service::start(&ctx, &name, cwd, &args)?;
            println!("Opened a terminal for {name}");
        }
        Cmd::Key { cmd } => match cmd {
            KeyCmd::Set { name } => {
                let k = secret_input(&format!("API key for {name}: "))?;
                service::set_api_key(&ctx, &name, &k)?;
                println!("Saved the key for {name}");
            }
            KeyCmd::Delete { name } => {
                hrm_core::secrets::delete(&hrm_core::secrets::key_target(&name))?;
                println!("Deleted the key for {name}");
            }
        },
        Cmd::Usage { days, profile, j } => {
            service::import_usage(&ctx)?;
            let s = service::usage_summary(&ctx, days, profile.as_deref())?;
            if j.json { return print_json(&s).map(|_| 0); }
            print!("{:<24}", "ACCOUNT");
            for d in &s.days { print!(" {:>8}", &d[5..]); }
            println!(" {:>9}", "TOTAL");
            for (p, v) in &s.by_profile {
                print!("{p:<24}");
                for x in v { print!(" {:>8}", fmt_tokens(*x)); }
                println!(" {:>9}", fmt_tokens(v.iter().sum()));
            }
            println!("Total {} tokens (input + output), cache reads {}", fmt_tokens(s.total), fmt_tokens(s.cache_read));
        }
        Cmd::Overview { days, harness, j } => {
            service::import_usage(&ctx)?;
            let o = service::overview(&ctx, days, harness.as_deref(), None)?;
            if j.json { return print_json(&o).map(|_| 0); }
            let money = |c: Option<f64>| c.map(|c| format!("${c:.2}")).unwrap_or_else(|| "-".into());
            println!("Cost (API list prices)  {}", money(o.cost));
            println!("Requests                {}", o.requests);
            println!("Tokens processed        {}", fmt_tokens(o.tokens));
            println!("Cache hit rate          {:.1}%", o.cache_hit_rate * 100.0);
            println!("Today                   {} · {} tokens", money(o.today_cost), fmt_tokens(o.today_tokens));
        }
        Cmd::Requests { days, harness, limit, j } => {
            service::import_usage(&ctx)?;
            let (total, rows) = service::request_log(&ctx, days, harness.as_deref(), limit, 0)?;
            if j.json { return print_json(&serde_json::json!({"total": total, "rows": rows})).map(|_| 0); }
            for r in rows {
                let t = chrono::DateTime::from_timestamp_millis(r.ts).map(|d| d.with_timezone(&chrono::Local).format("%m-%d %H:%M:%S").to_string()).unwrap_or_default();
                println!("{t}  {:<9} {:<22} {:<26} {:>9} {:>8} {:>8} {:>9}", r.harness, r.profile, r.model, r.cost.map(|c| format!("${c:.4}")).unwrap_or_else(|| "-".into()), r.input, r.output, r.cache_read);
            }
            println!("{total} requests in total");
        }
        Cmd::Quota { name, live, j } => {
            let reg = ctx.registry()?;
            let p = reg.get(&name)?;
            let q = if live {
                if p.harness != "claude" { bail!("live limits are only available for Claude subscription accounts"); }
                Some(harness::claude::Claude.live_quota(&ctx, p)?)
            } else {
                harness::of(p).quota(&ctx, p)
            };
            if j.json { return print_json(&q).map(|_| 0); }
            match q {
                Some(q) => for t in q.tiers { println!("{:<16} {:>3.0}%  resets {}", t.label, t.used, fmt_reset(t.resets_at)) },
                None => println!("No limits known yet for {name}"),
            }
        }
        Cmd::Sessions { group, harness: h, profile, days, search, open, project, tag, j } => {
            let f = SessionFilter { harness: h, profile, days, query: search, open: open.then_some(true), project, tag, scope: None };
            let groups = service::grouped_sessions(&ctx, &f, &group)?;
            if j.json { return print_json(&groups).map(|_| 0); }
            for g in groups {
                println!("{}  {}  ({} sessions, {} tokens)", g.name, g.root, g.sessions.len(), fmt_tokens(g.tokens));
                for s in g.sessions {
                    let when = chrono::DateTime::from_timestamp_millis(s.last_active_at).map(|d| d.with_timezone(&chrono::Local).format("%b %d %H:%M").to_string()).unwrap_or_default();
                    println!("  {:<8} {:<22} {:<12} {}{}  {}", s.harness, s.profile, when, if s.open { "* " } else { "" }, hrm_core::util::truncate(&s.title, 60), s.id);
                }
            }
        }
        Cmd::Session { profile, id, j } => {
            let t = service::transcript(&ctx, &profile, &id)?;
            if j.json { return print_json(&t).map(|_| 0); }
            println!("# {}\n{} · {} · {}\n", t.meta.title, t.meta.harness, t.meta.profile, t.meta.cwd);
            for turn in t.turns {
                if !turn.prompt.is_empty() { println!("> {}\n", turn.prompt); }
                for s in &turn.steps {
                    if s.kind == "thinking" { println!("  * thinking"); continue; }
                    let mark = match s.status.as_str() { "error" => "x", "ok" => "+", _ => "-" };
                    println!("  {mark} {}({}) {}", s.tool, s.input, s.result);
                }
                for a in &turn.answer { println!("\n{a}\n"); }
            }
        }
        // `hrm tag project github.com/o/r release` or `hrm tag session work-max/abc123 release bugfix`
        Cmd::Tag { kind, key, tags } => {
            if kind != "project" && kind != "session" { bail!("the kind is session or project"); }
            ctx.db.tag_set(&kind, &key, &tags)?;
            println!("{} {key}: {}", kind, if tags.is_empty() { "no tags".into() } else { tags.join(", ") });
        }
        Cmd::Tags { j } => {
            let list = ctx.db.tag_list()?;
            if j.json { return print_json(&list.iter().map(|(t, s, p)| serde_json::json!({"tag": t, "sessions": s, "projects": p})).collect::<Vec<_>>()).map(|_| 0); }
            for (t, s, p) in list { println!("{t:<24} {s:>4} sessions  {p:>3} projects"); }
        }
        Cmd::Resume { profile, id, cwd } => {
            let cwd = cwd.or_else(|| service::transcript(&ctx, &profile, &id).ok().map(|t| PathBuf::from(t.meta.cwd)).filter(|p| p.is_dir()));
            service::resume(&ctx, &profile, &id, cwd)?;
            println!("Resumed {id} in a new terminal");
        }
        Cmd::Projects { cmd } => match cmd {
            ProjectCmd::List { days, j } => {
                let list = service::projects(&ctx, days, None)?;
                if j.json { return print_json(&list).map(|_| 0); }
                println!("{:<28} {:>5}  {:>10}  {:>10}  {}", "project", "sess", "tokens", "list $", "key");
                for p in list { println!("{:<28} {:>5}  {:>10}  {:>10}  {}{}", p.name, p.sessions, fmt_tokens(p.tokens), p.cost.map(|c| format!("{c:.2}")).unwrap_or_else(|| "-".into()), p.key, if p.live > 0 { format!("  ({} live)", p.live) } else { String::new() }); }
            }
            ProjectCmd::Rename { key, name } => { service::alias_project(&ctx, &key, Some(name), None)?; println!("Renamed"); }
            ProjectCmd::Merge { key, into } => { service::alias_project(&ctx, &key, None, Some(into))?; println!("Merged"); }
            ProjectCmd::Reset { key } => { service::alias_project(&ctx, &key, None, None)?; println!("Reset"); }
        },
        Cmd::Sync { relink, j } => {
            let reg = ctx.registry()?;
            let mut relinked = vec![];
            if relink {
                let c = harness::claude::Claude;
                for p in reg.profiles.iter().filter(|p| p.harness == "claude" && !p.main) {
                    let done = hrm_core::shared::relink_copies(&harness::Harness::default_home(&c, &ctx), &harness::Harness::home(&c, &ctx, p))?;
                    if !done.is_empty() { relinked.push(format!("{}: {}", p.name, done.join(", "))); }
                }
            }
            let rep = items::sync(&ctx, &reg)?;
            if j.json { return print_json(&serde_json::json!({"relinked": relinked, "items": rep})).map(|_| 0); }
            for r in relinked { println!("relinked {r}"); }
            for w in &rep.written { println!("wrote    {w}"); }
            for w in &rep.removed { println!("removed  {w}"); }
            for w in &rep.conflicts { println!("conflict {w} (a different entry with that name exists; left alone)"); }
            for w in &rep.skipped { println!("skipped  {w}"); }
            println!("Accounts are in step.");
        }
        Cmd::Mcp { cmd } => return mcp(&ctx, cmd),
        Cmd::Skills { cmd } => return skills(&ctx, cmd),
        Cmd::Prompts { cmd } => return prompts(&ctx, cmd),
        Cmd::Provider { cmd } => {
            let mut reg = ctx.registry()?;
            match cmd {
                ProviderCmd::List { j } => {
                    if j.json { return print_json(&reg.providers).map(|_| 0); }
                    for p in &reg.providers { println!("{:<16} {:<10} {}  [{}]", p.id, p.format, p.base_url, p.models.join(", ")); }
                    // presets not added yet, with the command that adds them
                    for p in hrm_core::registry::provider_presets().into_iter().filter(|p| !reg.providers.iter().any(|q| q.id == p.id)) {
                        println!("{:<16} {:<10} {}  [{}]  preset: hrm provider add {} --base-url {} --format {} --models {}", p.id, p.format, p.base_url, p.models.join(", "), p.id, p.base_url, p.format, p.models.join(","));
                    }
                    return Ok(0);
                }
                ProviderCmd::Add { id, base_url, format, models, name } => {
                    if !hrm_core::util::valid_name(&id) { bail!("'{id}' is not a valid id"); }
                    if !matches!(format.as_str(), "anthropic" | "openai") { bail!("format must be anthropic or openai"); }
                    let key = secret_input(&format!("Key for {id}: "))?;
                    hrm_core::secrets::set_key(&hrm_core::secrets::provider_target(&id), key.trim())?;
                    reg.providers.retain(|p| p.id != id);
                    reg.providers.push(Provider { name: name.unwrap_or_else(|| id.clone()), id: id.clone(), base_url, format, models });
                    println!("Added provider {id}");
                }
                ProviderCmd::Remove { id } => {
                    reg.remove_provider(&id)?;
                    hrm_core::secrets::delete(&hrm_core::secrets::provider_target(&id))?;
                    println!("Removed provider {id}");
                }
            }
            ctx.save_registry(&reg)?;
        }
        Cmd::Route { cmd } => {
            let mut reg = ctx.registry()?;
            match cmd {
                RouteCmd::List { j } => {
                    if j.json { return print_json(&reg.routes).map(|_| 0); }
                    for r in &reg.routes { println!("{:<16} {:<10} {}", r.id, r.mode, r.providers.join(" → ")); }
                    return Ok(0);
                }
                RouteCmd::Add { id, mode, providers, default } => {
                    if !hrm_core::util::valid_name(&id) { bail!("'{id}' is not a valid id"); }
                    if !matches!(mode.as_str(), "failover" | "aggregate") { bail!("mode must be failover or aggregate"); }
                    for p in &providers { if !reg.providers.iter().any(|x| &x.id == p) { bail!("no provider named '{p}'"); } }
                    if providers.is_empty() { bail!("give at least one provider"); }
                    reg.routes.retain(|r| r.id != id);
                    reg.routes.push(Route { id: id.clone(), mode, providers, default_provider: default });
                    println!("Added route {id}");
                }
                RouteCmd::Remove { id } => { reg.remove_route(&id)?; println!("Removed route {id}"); }
            }
            ctx.save_registry(&reg)?;
        }
        Cmd::Router { cmd } => return router_cmd::run(&ctx, cmd),
        Cmd::Import { path, j } => {
            let mut reg = ctx.registry()?;
            let mut rep = hrm_core::import::ImportReport::default();
            let path = path.unwrap_or_else(|| hrm_core::import::default_ps_registry(&ctx));
            hrm_core::import::import_powershell(&ctx, &mut reg, &path, &mut rep)?;
            ctx.save_registry(&reg)?;
            for p in &reg.profiles { let _ = harness::of(p).prepare(&ctx, p); }
            if j.json { return print_json(&rep).map(|_| 0); }
            println!("Imported {} accounts and {} keys", rep.accounts.len(), rep.keys.len());
            for s in rep.skipped { println!("  skipped {s}"); }
        }
        Cmd::Init { j } => {
            let mut reg = ctx.registry()?;
            let mut rep = hrm_core::import::ImportReport::default();
            hrm_core::import::detect_main_profiles(&ctx, &mut reg, &mut rep);
            ctx.save_registry(&reg)?;
            if j.json { return print_json(&rep).map(|_| 0); }
            println!("Main profiles added: {}", if rep.main_profiles.is_empty() { "none".into() } else { rep.main_profiles.join(", ") });
        }
        Cmd::Harnesses { j } => {
            let hs = service::harnesses();
            if j.json { return print_json(&hs).map(|_| 0); }
            for h in hs { println!("{:<10} {:<12} {}", h.id, h.label, if h.installed { "installed" } else { "not installed" }); }
        }
        Cmd::Settings { key, value } => {
            let mut ctx = ctx;
            let mut v = serde_json::to_value(&ctx.settings)?;
            match (key, value) {
                (None, _) => println!("{}", serde_json::to_string_pretty(&v)?),
                (Some(k), None) => println!("{}", v.get(&k).context("no such setting")?),
                (Some(k), Some(val)) => {
                    let cur = v.get(&k).context("no such setting")?.clone();
                    v[&k] = if cur.is_number() { serde_json::json!(val.parse::<u64>()?) } else { serde_json::json!(val) };
                    ctx.settings = serde_json::from_value(v)?;
                    ctx.save_settings()?;
                    println!("{k} = {val}");
                }
            }
        }
        Cmd::Open => {
            let app = hrm_core::launch::hrm_exe().with_file_name(if cfg!(windows) { "HarnessManager.exe" } else { "harnessmanager" });
            std::process::Command::new(&app).spawn().with_context(|| format!("start {}", app.display()))?;
        }
        Cmd::Computers { cmd } => {
            let on_off = |s: &Option<String>| -> Result<Option<bool>> {
                Ok(match s.as_deref() { None => None, Some("on") => Some(true), Some("off") => Some(false), Some(x) => bail!("'{x}' is not on or off") })
            };
            let (st, j) = match cmd {
                ComputersCmd::Status { j } => (hrm_core::journal::status(&ctx)?, j),
                ComputersCmd::Now { j } => (hrm_core::journal::round(&ctx)?, j),
                ComputersCmd::Set { name, mode, folder, transcripts, presence, direct, port, j } => {
                    let args = serde_json::json!({"device_name": name, "sync_mode": mode, "sync_folder": folder.map(|f| f.display().to_string()),
                        "sync_transcripts": on_off(&transcripts)?, "sync_presence": on_off(&presence)?, "direct_connection": on_off(&direct)?, "listen_port": port});
                    let fresh = hrm_core::journal::configure(&ctx.data, &ctx.home, &args)?;
                    (if fresh.settings.sync_mode == "folder" { hrm_core::journal::round(&fresh)? } else { hrm_core::journal::status(&fresh)? }, j)
                }
                ComputersCmd::Forget { id } => (hrm_core::sync::peer::forget(&ctx, &id)?, Json { json: false }),
                ComputersCmd::Peers { j } => {
                    let st = hrm_core::journal::status(&ctx)?;
                    if j.json { return print_json(&st["peers"]).map(|_| 0); }
                    for p in st["peers"].as_array().into_iter().flatten() {
                        let how = if p["direct"] == true { if p["connected"] == true { format!("connected by {}", p["path"].as_str().unwrap_or("")) } else { "paired, not reached".into() } } else { "folder only".into() };
                        println!("{:<24} {:<38} {:<24} address {}", p["name"].as_str().unwrap_or(""), p["id"].as_str().unwrap_or(""), how, p["address"].as_str().filter(|a| !a.is_empty()).unwrap_or("-"));
                    }
                    return Ok(0);
                }
                ComputersCmd::Pair { host } => {
                    let (rt, ctx) = peers_runtime(ctx)?;
                    let v = rt.pair_start(&ctx, host.as_deref().filter(|h| !h.is_empty()))?;
                    println!("code     {}", v["code"].as_str().unwrap_or(""));
                    println!("host     {}", v["host_hint"].as_str().unwrap_or(""));
                    println!("waiting two minutes for the other computer to join");
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    let mut n = 0;
                    let code = loop {
                        match rt.pair_result() {
                            Some(Some(r)) if r["ok"] == true => { rt.tick(&ctx); println!("paired with {}", r["peer"]["name"].as_str().unwrap_or("")); break 0 }
                            Some(Some(r)) => { eprintln!("{}", r["error"].as_str().unwrap_or("the pairing ended")); break 1 }
                            None => { eprintln!("the code expired"); break 1 }
                            Some(None) => {}
                        }
                        if n % 20 == 0 { rt.tick(&ctx); }
                        n += 1;
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    };
                    rt.stop();
                    return Ok(code);
                }
                ComputersCmd::Join { code, host } => {
                    let (rt, ctx) = peers_runtime(ctx)?;
                    let st = rt.pair_join(&ctx, &code, host.as_deref())?;
                    let name = st["peers"].as_array().into_iter().flatten().filter(|p| p["paired_at"].is_i64()).max_by_key(|p| p["paired_at"].as_i64()).and_then(|p| p["name"].as_str()).unwrap_or("");
                    println!("paired with {name}");
                    rt.stop();
                    return Ok(0);
                }
                ComputersCmd::Serve { seconds } => {
                    let (rt, ctx) = peers_runtime(ctx)?;
                    let until = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
                    rt.tick(&ctx);
                    println!("listening on port {} for {seconds} s", hrm_core::Ctx::open_at(&ctx.data, &ctx.home)?.settings.listen_port);
                    use std::io::Write;
                    let _ = std::io::stdout().flush();
                    while std::time::Instant::now() < until {
                        std::thread::sleep(std::time::Duration::from_secs(10).min(until.saturating_duration_since(std::time::Instant::now())));
                        rt.tick(&ctx);
                    }
                    rt.stop();
                    return Ok(0);
                }
            };
            if j.json { return print_json(&st).map(|_| 0); }
            let t = &st["this"];
            println!("{}  ({})  mode {}{}", t["name"].as_str().unwrap_or(""), t["id"].as_str().unwrap_or(""), st["mode"].as_str().unwrap_or("off"),
                st["folder"].as_str().filter(|f| !f.is_empty()).map(|f| format!("  folder {f}")).unwrap_or_default());
            println!("  journal    {} entries, {} published", t["last_seq"], t["published_seq"]);
            if let Some(e) = st["error"].as_str() { println!("  last run   {}", e); } else if let Some(at) = st["last_run"].as_i64() { println!("  last run   {}", fmt_reset(Some(at))); }
            for p in st["peers"].as_array().into_iter().flatten() {
                println!("  {:<24} {:<8} {:<8} seen {}  applied {}/{}  {} running", p["name"].as_str().unwrap_or(""), p["os"].as_str().unwrap_or(""), p["app_version"].as_str().unwrap_or(""),
                    fmt_reset(p["last_seen"].as_i64()), p["applied_seq"], p["last_seq"], p["running"].as_array().map(|r| r.len()).unwrap_or(0));
            }
        }
        Cmd::ResumeHere { session } => {
            let parts: Vec<&str> = session.splitn(4, '/').collect();
            let [computer, harness, profile, id] = parts[..] else { bail!("give <computer>/<tool>/<account>/<session id>") };
            // the computer by name or id
            let names = hrm_core::journal::device_names(&ctx);
            let device = names.iter().find(|(id, n)| *id == computer || n.eq_ignore_ascii_case(computer)).map(|(id, _)| id.clone()).with_context(|| format!("no computer named {computer}"))?;
            let r = hrm_core::transcripts::resume_here(&ctx, &device, harness, profile, id)?;
            println!("Resumed {id} from {computer} in {}: {}", r["cwd"].as_str().unwrap_or(""), r["note"].as_str().unwrap_or(""));
        }
        Cmd::Transcripts { j } => {
            let st = hrm_core::transcripts::status(&ctx)?;
            if j.json { return print_json(&st).map(|_| 0); }
            println!("carried {} ({:.1} MB of {} MB); from the other computers {} known, {} fetched", st["carried"], st["bytes"].as_f64().unwrap_or(0.0) / 1048576.0, st["cap_bytes"].as_i64().unwrap_or(0) / 1048576, st["remote"], st["fetched"]);
            for s in st["skipped"].as_array().into_iter().flatten() { println!("  skipped {}  {}", s["key"].as_str().unwrap_or(""), s["why"].as_str().unwrap_or("")); }
        }
        Cmd::Backup { with_transcripts, j } => {
            let b = hrm_core::backup::create_with(&ctx, "Manual", with_transcripts)?;
            if j.json { return print_json(&b).map(|_| 0); }
            println!("Backed up: {} ({:.1} MB){}", b.name, b.size as f64 / 1_048_576.0, if with_transcripts { ", with transcripts" } else { "" });
        }
        Cmd::Restore { source, adopt, preview, j } => {
            let src = hrm_core::restore::Source::of_path(&source)?;
            let v = if preview { hrm_core::restore::preview(&ctx, &src)? } else { hrm_core::restore::run(&ctx, &src, adopt)? };
            if j.json { return print_json(&v).map(|_| 0); }
            if preview {
                println!("{} requests, {} tokens, {} sessions, {} accounts with limit history, {} archived transcripts, newest {}", v["requests"], fmt_tokens(v["tokens"].as_i64().unwrap_or(0)), v["sessions"], v["accounts"], v["archived"], fmt_reset(v["newest"].as_i64()));
                for c in v["computers"].as_array().into_iter().flatten() { println!("  {:<24} {} requests, {} sessions", c["name"].as_str().unwrap_or(""), c["requests"], c["sessions"]); }
                if let Some(n) = v["this"]["name"].as_str() { println!("  {n} is this computer's name: --adopt continues as it"); }
            } else {
                println!("Restored: {} requests, {} sessions, {} limit readings, {} tags, {} computers, {} archived transcripts, {} accounts added{}", v["usage"], v["sessions"], v["quota"], v["tags"], v["peers"], v["archived"], v["profiles"],
                    if v["adopted"] == true { format!("; this computer continues as {}", v["device"]["name"].as_str().unwrap_or("")) } else { String::new() });
            }
        }
        Cmd::Archive { cmd } => {
            let (v, j) = match cmd {
                ArchiveCmd::Status { j } => (hrm_core::archive::status(&ctx)?, j),
                ArchiveCmd::Now { j } => (hrm_core::archive::archive_now(&ctx)?, j),
            };
            if j.json { return print_json(&v).map(|_| 0); }
            if v.get("locations").is_some() {
                println!("{} archived ({:.1} MB); sessions older than {} days{}", v["archived"], v["bytes"].as_f64().unwrap_or(0.0) / 1_048_576.0, v["days"], if v["days"] == 0 { " (never)" } else { "" });
                for l in v["locations"].as_array().into_iter().flatten() { println!("  {:<12} {} ({} sessions, {:.1} MB){}", l["kind"].as_str().unwrap_or(""), l["path"].as_str().unwrap_or(""), l["sessions"], l["bytes"].as_f64().unwrap_or(0.0) / 1_048_576.0, if l["on"] == false { ", off" } else { "" }); }
                if let Some(at) = v["last"]["at"].as_i64() { println!("  last run {}: {} moved", fmt_reset(Some(at)), v["last"]["archived"]); }
            } else {
                println!("{} {} moved to the archive ({:.1} MB){}", v["archived"], if v["archived"] == 1 { "session" } else { "sessions" }, v["bytes"].as_f64().unwrap_or(0.0) / 1_048_576.0, v["why"].as_str().map(|w| format!("; {w}")).unwrap_or_default());
            }
        }
    }
    Ok(0)
}

/// The direct connection's runtime for one CLI command, bound to every interface; the setting is turned on.
fn peers_runtime(ctx: Ctx) -> Result<(hrm_core::sync::peer::Runtime, std::sync::Arc<Ctx>)> {
    let ctx = if ctx.settings.direct_connection { ctx } else { hrm_core::journal::configure(&ctx.data, &ctx.home, &serde_json::json!({"direct_connection": true}))? };
    let rt = hrm_core::sync::peer::Runtime::new(std::net::Ipv4Addr::UNSPECIFIED.into(), std::sync::Arc::new(|_, _| {}))?;
    Ok((rt, std::sync::Arc::new(ctx)))
}

fn login(ctx: &Ctx, name: &str) -> Result<i32> {
    let reg = ctx.registry()?;
    let p = reg.get(name)?.clone();
    let h = harness::of(&p);
    h.prepare(ctx, &p)?;
    if p.harness == "agy" { harness::agy::Agy.prepare_login(ctx)?; }
    let code = h.login(ctx, &p)?.run()?;
    if p.harness == "agy" && !harness::agy::Agy.finish_login(ctx, &p)? {
        eprintln!("Antigravity did not save a login; run `hrm login {name}` again.");
        return Ok(1);
    }
    let id = h.identity(ctx, &p);
    if id.logged_in {
        println!("{name} is logged in{}", id.email.map(|e| format!(" as {e}")).unwrap_or_default());
        if let (Some(want), Some(got)) = (&p.email, &h.identity(ctx, &p).email) {
            if !want.eq_ignore_ascii_case(got) { eprintln!("warning: you entered {want} but logged in as {got}"); }
        }
    }
    Ok(code)
}

fn targets_for(reg: &hrm_core::Registry, kind: &str, item: &str) -> String {
    let t: Vec<String> = reg.targets.iter().filter(|t| t.kind == kind && t.item == item).map(|t| match &t.profile { Some(p) => format!("{}:{p}", t.harness), None => t.harness.clone() }).collect();
    if t.is_empty() { "off".into() } else { t.join(", ") }
}

fn mcp(ctx: &Ctx, cmd: McpCmd) -> Result<i32> {
    let mut reg = ctx.registry()?;
    match cmd {
        McpCmd::List { j } => {
            if j.json { return print_json(&serde_json::json!({"servers": reg.mcp, "targets": reg.targets.iter().filter(|t| t.kind == "mcp").collect::<Vec<_>>()})).map(|_| 0); }
            for s in &reg.mcp { println!("{:<20} {:<6} {:<40} {}", s.name, s.transport, s.command.clone().or(s.url.clone()).unwrap_or_default(), targets_for(&reg, "mcp", &s.name)); }
            return Ok(0);
        }
        McpCmd::Add { name, command, args, url, transport, env, headers } => {
            let transport = if transport.is_empty() { if url.is_some() { "http".into() } else { "stdio".into() } } else { transport };
            items::add_mcp(&mut reg, McpServer { name: name.clone(), transport, command, args, env: parse_pairs(&env, '=')?, url, headers: parse_pairs(&headers, ':')?, ..Default::default() })?;
            println!("Added {name}. Enable it with `hrm mcp enable {name} <tool>`.");
        }
        McpCmd::Remove { name } => { items::remove_item(&mut reg, "mcp", &name)?; println!("Removed {name}"); }
        McpCmd::Enable { name, harness: h, profile } => { items::set_target(&mut reg, "mcp", &name, &h, profile.as_deref(), true)?; }
        McpCmd::Disable { name, harness: h, profile } => { items::set_target(&mut reg, "mcp", &name, &h, profile.as_deref(), false)?; }
        McpCmd::Import { harness: h } => {
            let (added, kept) = items::import_mcp(ctx, &mut reg, &h)?;
            println!("Imported {} server(s): {}", added.len(), added.join(", "));
            if !kept.is_empty() { println!("Left alone, the library has a different server with that name: {}", kept.join(", ")); }
        }
    }
    ctx.save_registry(&reg)?;
    let rep = items::sync(ctx, &reg)?;
    for c in rep.conflicts { eprintln!("conflict: {c} (a different entry with that name exists; left alone)"); }
    for s in rep.skipped { eprintln!("skipped: {s}"); }
    Ok(0)
}

fn skills(ctx: &Ctx, cmd: SkillCmd) -> Result<i32> {
    let mut reg = ctx.registry()?;
    match cmd {
        SkillCmd::List { j } => {
            if j.json { return print_json(&serde_json::json!({"skills": reg.skills, "targets": reg.targets.iter().filter(|t| t.kind == "skill").collect::<Vec<_>>()})).map(|_| 0); }
            for s in &reg.skills { println!("{:<24} {:<50} {}", s.name, hrm_core::util::truncate(&s.source, 50), targets_for(&reg, "skill", &s.name)); }
            return Ok(0);
        }
        SkillCmd::Install { source, name } => { let s = items::install_skill(ctx, &mut reg, &source, name.as_deref())?; println!("Installed {}", s.name); }
        SkillCmd::Remove { name } => {
            items::remove_item(&mut reg, "skill", &name)?;
            ctx.save_registry(&reg)?;
            items::sync(ctx, &reg)?;
            items::delete_skill_files(ctx, &name)?;
            println!("Removed {name}");
            return Ok(0);
        }
        SkillCmd::Enable { name, harness: h, profile } => { items::set_target(&mut reg, "skill", &name, &h, profile.as_deref(), true)?; }
        SkillCmd::Disable { name, harness: h, profile } => { items::set_target(&mut reg, "skill", &name, &h, profile.as_deref(), false)?; }
        SkillCmd::Import { harness: h } => {
            let (a, kept) = items::import_skills(ctx, &mut reg, &h)?;
            println!("Imported {} skill(s): {}", a.len(), a.join(", "));
            if !kept.is_empty() { println!("Left alone, a different skill with that name is already installed: {}", kept.join(", ")); }
        }
        SkillCmd::Check => {
            for (n, upd) in items::check_skill_updates(ctx, &reg) { println!("{n:<24} {}", match upd { Some(true) => "update available", Some(false) => "up to date", None => "could not check (source not reachable)" }); }
            return Ok(0);
        }
    }
    ctx.save_registry(&reg)?;
    let rep = items::sync(ctx, &reg)?;
    for c in rep.conflicts { eprintln!("conflict: {c}"); }
    Ok(0)
}

fn prompts(ctx: &Ctx, cmd: PromptCmd) -> Result<i32> {
    let mut reg = ctx.registry()?;
    match cmd {
        PromptCmd::List { j } => {
            if j.json { return print_json(&reg.prompts).map(|_| 0); }
            for p in &reg.prompts { println!("{:<18} {:<10} {:<4} {}", p.id, p.harness, if p.enabled { "on" } else { "" }, p.title); }
            return Ok(0);
        }
        PromptCmd::Add { harness: h, title, file } => { let id = items::add_prompt(&mut reg, &h, &title, &std::fs::read_to_string(file)?)?; println!("Added {id}"); }
        PromptCmd::Enable { id } => { items::enable_prompt(ctx, &mut reg, &id)?; println!("Enabled {id}"); }
        PromptCmd::Show { id } => { println!("{}", reg.prompts.iter().find(|p| p.id == id).context("no such prompt")?.body); return Ok(0); }
    }
    ctx.save_registry(&reg)?;
    Ok(0)
}

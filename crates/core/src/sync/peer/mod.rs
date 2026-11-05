//! The direct connection: paired computers talk over mutual TLS, found by mDNS or an address, pulling each
//! other's journals with cursors (and relaying those of computers they both know) and presence every ten
//! seconds. The folder keeps working underneath. `Runtime` owns the listener, the mDNS daemon and a pairing in
//! progress for one process (the app or `hrm computers serve`); the pulls need none of it.

pub mod client;
pub mod identity;
pub mod listener;
pub mod mdns;
pub mod pairing;
pub mod store;
#[cfg(test)]
mod tests;

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

pub use client::pull_all;

use crate::util::now_ms;
use crate::{journal, Ctx};

/// Told of what happens (`computers-pairing` with `{ok, peer?, error?}`); the app forwards it to the window.
pub type OnEvent = Arc<dyn Fn(&str, Value) + Send + Sync>;

pub struct Runtime {
    rt: tokio::runtime::Runtime,
    bind: IpAddr,
    listener: Mutex<Option<listener::Listener>>,
    mdns: Mutex<Option<mdns::Mdns>>,
    host: Mutex<Option<pairing::Host>>,
    on_event: OnEvent,
}

impl Runtime {
    /// `bind`: every interface for the app and the CLI, loopback in tests.
    pub fn new(bind: IpAddr, on_event: OnEvent) -> Result<Runtime> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
        Ok(Runtime { rt, bind, listener: Mutex::new(None), mdns: Mutex::new(None), host: Mutex::new(None), on_event })
    }

    /// The listener and mDNS follow the setting. Returns the context to go on with: a fresh one when the port
    /// was just chosen and saved.
    pub fn ensure(&self, ctx: &Arc<Ctx>) -> Result<Arc<Ctx>> {
        let mut l = self.listener.lock().unwrap();
        if !ctx.settings.direct_connection {
            if let Some(lis) = l.take() { lis.stop(); crate::log::info("computers: the direct connection is off"); }
            if let Some(m) = self.mdns.lock().unwrap().take() { m.stop(); }
            return Ok(ctx.clone());
        }
        if l.is_some() { return Ok(ctx.clone()); }
        let lis = listener::start(ctx.clone(), self.rt.handle(), self.bind, ctx.settings.listen_port)?;
        let mut ctx = ctx.clone();
        if ctx.settings.listen_port != lis.port {
            let mut c = Ctx::open_at(&ctx.data, &ctx.home)?;
            c.settings.listen_port = lis.port;
            c.save_settings()?;
            ctx = Arc::new(c);
        }
        crate::log::info(&format!("computers: listening for paired computers on port {}", lis.port));
        match mdns::Mdns::start(&ctx.settings.device_id, &ctx.settings.device_name, lis.port) {
            Ok(m) => *self.mdns.lock().unwrap() = Some(m),
            Err(e) => crate::log::warn(&format!("computers: mDNS is not available: {e}")),
        }
        *l = Some(lis);
        Ok(ctx)
    }

    /// Every ten seconds while on: the listener up, what is new here published, every peer pulled.
    pub fn tick(&self, ctx: &Arc<Ctx>) {
        let done = self.host.lock().unwrap().as_ref().map(|h| h.result().is_some() || h.expires_at < now_ms()).unwrap_or(false);
        if done { self.pair_cancel_inner(); }
        match self.ensure(ctx) {
            Ok(ctx) if ctx.settings.direct_connection => {
                if let Err(e) = journal::publish(&ctx) { crate::log::warn(&format!("journal: {e:#}")); }
                pull_all(&ctx);
            }
            Ok(_) => {}
            Err(e) => crate::log::warn(&format!("computers: the direct connection could not start: {e:#}")),
        }
    }

    pub fn stop(&self) {
        self.pair_cancel_inner();
        if let Some(l) = self.listener.lock().unwrap().take() { l.stop(); }
        if let Some(m) = self.mdns.lock().unwrap().take() { m.stop(); }
    }

    /// The direct connection on (saved) and the listener up, for the pairing ops.
    fn ready(&self, ctx: &Arc<Ctx>) -> Result<Arc<Ctx>> {
        let ctx = if ctx.settings.direct_connection { ctx.clone() } else { Arc::new(journal::configure(&ctx.data, &ctx.home, &json!({"direct_connection": true}))?) };
        self.ensure(&ctx)
    }

    /// Show a code for two minutes; `host_name` goes into the host hint instead of this computer's name.
    pub fn pair_start(&self, ctx: &Arc<Ctx>, host_name: Option<&str>) -> Result<Value> {
        let ctx = self.ready(ctx)?;
        self.pair_cancel_inner();
        let h = pairing::start(ctx.clone(), self.rt.handle(), self.bind, host_name, self.on_event.clone())?;
        if let Some(m) = self.mdns.lock().unwrap().as_mut() { let _ = m.advertise(Some(h.port)); }
        let v = h.status();
        *self.host.lock().unwrap() = Some(h);
        Ok(v)
    }

    fn pair_cancel_inner(&self) {
        if let Some(h) = self.host.lock().unwrap().take() {
            h.cancel();
            if let Some(m) = self.mdns.lock().unwrap().as_mut() { let _ = m.advertise(None); }
        }
    }

    pub fn pair_cancel(&self, ctx: &Ctx) -> Result<Value> {
        self.pair_cancel_inner();
        journal::status(ctx)
    }

    /// Join the computer showing `code`, then pull it at once so it shows connected.
    pub fn pair_join(&self, ctx: &Arc<Ctx>, code: &str, host: Option<&str>) -> Result<Value> {
        let ctx = self.ready(ctx)?;
        pairing::join(&ctx, code, host)?;
        pull_all(&ctx);
        journal::status(&ctx)
    }

    /// The pairing shown now: `{ok, peer?, error?}` once it ended, `null` while it waits; `None` without one.
    pub fn pair_result(&self) -> Option<Option<Value>> { self.host.lock().unwrap().as_ref().map(|h| h.result()) }
}

/// A peer's id from its id or name.
fn peer_id(ctx: &Ctx, id_or_name: &str) -> Result<String> {
    let peers = journal::peers(ctx)?;
    let hit: Vec<&journal::Peer> = peers.iter().filter(|p| p.id == id_or_name || p.name.eq_ignore_ascii_case(id_or_name)).collect();
    match hit.as_slice() {
        [p] => Ok(p.id.clone()),
        [] => bail!("no computer is called {id_or_name}"),
        _ => bail!("{id_or_name} names more than one computer; give its id"),
    }
}

/// Forget a paired computer: its row and certificate go, what was merged from it stays; its next `hello` here
/// is refused.
pub fn forget(ctx: &Ctx, id_or_name: &str) -> Result<Value> {
    let id = peer_id(ctx, id_or_name)?;
    ctx.db.with(|c| { c.execute("delete from peers where device_id=?", [&id])?; Ok(()) })?;
    let _ = std::fs::remove_dir_all(store::dir(ctx, &id));
    crate::log::info(&format!("computers: {id} forgotten"));
    journal::status(ctx)
}

/// A peer's address (a VPN name, `name` or `name:port`) or display name.
pub fn set_peer(ctx: &Ctx, id: &str, address: Option<&str>, name: Option<&str>) -> Result<Value> {
    let id = peer_id(ctx, id)?;
    if let Some(a) = address {
        ctx.db.with(|c| { c.execute("update peers set address=? where device_id=?", [a.trim(), &id])?; Ok(()) })?;
    }
    if let Some(n) = name {
        let n = n.trim();
        if n.is_empty() || n.chars().count() > 60 { bail!("give the computer a name of up to 60 characters"); }
        ctx.db.with(|c| { c.execute("update peers set name=? where device_id=?", [n, &id])?; Ok(()) })?;
    }
    journal::status(ctx).context("status")
}

//! The direct connection: paired computers talk over mutual TLS, found by mDNS or an address, pulling each
//! other's journals with cursors (and relaying those of computers they both know) and presence every ten
//! seconds. The folder keeps working underneath. `Runtime` owns the listener, the mDNS daemon and a pairing in
//! progress for one process (the app or `hrm computers serve`), a computer's or a phone's; the pulls need none of it.
//! Phones are served by the same listener (`crate::companion`).

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

#[cfg(test)]
type Seam = Option<Box<dyn FnOnce(&Runtime)>>;

#[cfg(test)]
thread_local! {
    /// Test seam: the next registrations mDNS refuses.
    pub(crate) static FAILED_REGISTRATIONS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
thread_local! {
    /// Test seam: runs where a phone pairing decision has looked and not yet acted (with `phone` held, so nothing else
    /// can change the attempt there).
    pub(crate) static PHONE_SEAM: std::cell::RefCell<Seam> = std::cell::RefCell::new(None);
}

#[cfg(test)]
fn phone_seam(rt: &Runtime) { if let Some(f) = PHONE_SEAM.with(|s| s.borrow_mut().take()) { f(rt); } }

/// Told of what happens (`computers-pairing` with `{ok, peer?, error?}`, `phones-pairing` with the phone pairing's
/// status); the app forwards it to the window.
pub type OnEvent = Arc<dyn Fn(&str, Value) + Send + Sync>;

pub struct Runtime {
    rt: tokio::runtime::Runtime,
    bind: IpAddr,
    listener: Mutex<Option<listener::Listener>>,
    mdns: Mutex<Option<mdns::Mdns>>,
    host: Mutex<Option<pairing::Host>>,
    phone: Mutex<Option<pairing::PhoneHost>>,
    /// The phone pairing port mDNS announces now; only changed while `phone` is held, so it follows the attempt shown.
    /// With whether mDNS took the registration: one that failed is tried again at each tick while the attempt lasts.
    phone_announced: Mutex<Option<(u16, bool)>>,
    on_event: OnEvent,
}

impl Runtime {
    /// `bind`: every interface for the app and the CLI, loopback in tests.
    pub fn new(bind: IpAddr, on_event: OnEvent) -> Result<Runtime> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build()?;
        Ok(Runtime { rt, bind, listener: Mutex::new(None), mdns: Mutex::new(None), host: Mutex::new(None), phone: Mutex::new(None), phone_announced: Mutex::new(None), on_event })
    }

    /// The listener and mDNS follow the setting. Returns the context to go on with: a fresh one when the port
    /// was just chosen and saved.
    pub fn ensure(&self, ctx: &Arc<Ctx>) -> Result<Arc<Ctx>> {
        let mut l = self.listener.lock().unwrap_or_else(|e| e.into_inner());
        if !ctx.settings.direct_connection {
            if let Some(lis) = l.take() { lis.stop(); crate::log::info("computers: the direct connection is off"); }
            if let Some(m) = self.mdns.lock().unwrap_or_else(|e| e.into_inner()).take() { m.stop(); }
            return Ok(ctx.clone());
        }
        if let Some(lis) = l.as_ref() {
            // mDNS that could not start is started again: a phone given the six digits finds the computer through it
            if !self.bind.is_loopback() && self.mdns.lock().unwrap_or_else(|e| e.into_inner()).is_none() { self.start_mdns(ctx, lis.port); }
            // a context opened before the listener chose its port says 0: the one going on says the port it holds
            if ctx.settings.listen_port != lis.port {
                let mut c = Ctx::open_at(&ctx.data, &ctx.home)?;
                c.settings.listen_port = lis.port;
                return Ok(Arc::new(c));
            }
            return Ok(ctx.clone());
        }
        let lis = listener::start(ctx.clone(), self.rt.handle(), self.bind, ctx.settings.listen_port)?;
        let mut ctx = ctx.clone();
        if ctx.settings.listen_port != lis.port {
            let mut c = Ctx::open_at(&ctx.data, &ctx.home)?;
            c.settings.listen_port = lis.port;
            c.save_settings()?;
            ctx = Arc::new(c);
        }
        crate::log::info(&format!("computers: listening for paired computers on port {}", lis.port));
        // a loopback-only listener (the tests) cannot be reached at the addresses mDNS would announce
        if !self.bind.is_loopback() { self.start_mdns(&ctx, lis.port); }
        *l = Some(lis);
        Ok(ctx)
    }

    fn start_mdns(&self, ctx: &Ctx, port: u16) {
        match mdns::Mdns::start(&ctx.settings.device_id, &ctx.settings.device_name, port) {
            Ok(mut m) => {
                // what was shown before mDNS came up is announced now
                let announced = *self.phone_announced.lock().unwrap_or_else(|e| e.into_inner());
                if let Some((p, _)) = announced { let _ = m.advertise_phone(Some(p)); }
                *self.mdns.lock().unwrap_or_else(|e| e.into_inner()) = Some(m);
            }
            Err(e) => crate::log::warn(&format!("computers: mDNS is not available: {e}")),
        }
    }

    /// Every ten seconds while on: the listener up, what is new here published, every peer pulled.
    pub fn tick(&self, ctx: &Arc<Ctx>) {
        let done = self.host.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|h| h.result().is_some() || h.expires_at < now_ms()).unwrap_or(false);
        if done { self.pair_cancel_inner(); }
        // a phone pairing that ended keeps its status for the window, but is no longer announced
        self.phone_tick();
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
        self.phone_cancel_inner();
        if let Some(l) = self.listener.lock().unwrap_or_else(|e| e.into_inner()).take() { l.stop(); }
        if let Some(m) = self.mdns.lock().unwrap_or_else(|e| e.into_inner()).take() { m.stop(); }
    }

    /// The direct connection on (saved) and the listener up, for the pairing ops. When another process already
    /// holds this computer's listener (the app, or `hrm computers serve`, beside `hrm phones pair`), pairing goes on
    /// beside it: the phone is given that listener's port, which it saved.
    fn ready(&self, ctx: &Arc<Ctx>) -> Result<Arc<Ctx>> {
        let ctx = if ctx.settings.direct_connection { ctx.clone() } else { Arc::new(journal::configure(&ctx.data, &ctx.home, &json!({"direct_connection": true}))?) };
        match self.ensure(&ctx) {
            Err(e) if ctx.settings.listen_port != 0 && e.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some_and(|io| io.kind() == std::io::ErrorKind::AddrInUse)) => {
                crate::log::info(&format!("computers: port {} is held by this computer's running listener; pairing beside it", ctx.settings.listen_port));
                Ok(ctx)
            }
            r => r,
        }
    }

    /// The port this computer's listener takes phones on: its own listener's, or the one another process holds
    /// (saved in the settings); 0 when there is none.
    fn listen_port(&self, ctx: &Ctx) -> u16 {
        if let Some(l) = self.listener.lock().unwrap_or_else(|e| e.into_inner()).as_ref() { return l.port; }
        Ctx::open_at(&ctx.data, &ctx.home).map(|c| c.settings.listen_port).unwrap_or(ctx.settings.listen_port)
    }

    /// Show a code for two minutes; `host_name` goes into the host hint instead of this computer's name.
    pub fn pair_start(&self, ctx: &Arc<Ctx>, host_name: Option<&str>) -> Result<Value> {
        let ctx = self.ready(ctx)?;
        self.pair_cancel_inner();
        let h = pairing::start(ctx.clone(), self.rt.handle(), self.bind, host_name, self.on_event.clone())?;
        if let Some(m) = self.mdns.lock().unwrap_or_else(|e| e.into_inner()).as_mut() { let _ = m.advertise(Some(h.port)); }
        let v = h.status();
        *self.host.lock().unwrap_or_else(|e| e.into_inner()) = Some(h);
        Ok(v)
    }

    fn pair_cancel_inner(&self) {
        if let Some(h) = self.host.lock().unwrap_or_else(|e| e.into_inner()).take() {
            h.cancel();
            if let Some(m) = self.mdns.lock().unwrap_or_else(|e| e.into_inner()).as_mut() { let _ = m.advertise(None); }
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
    pub fn pair_result(&self) -> Option<Option<Value>> { self.host.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|h| h.result()) }

    /// Announce the port of the phone pairing shown, or none; the caller holds `phone`, so the port is the attempt's.
    fn advertise_phone(&self, _shown: &Option<pairing::PhoneHost>, port: Option<u16>) {
        let registered = self.mdns.lock().unwrap_or_else(|e| e.into_inner()).as_mut().map(|m| m.advertise_phone(port).is_ok());
        let took = registration_took(registered, self.bind.is_loopback());
        #[cfg(test)]
        let took = took && !FAILED_REGISTRATIONS.with(|n| { let failing = n.get() > 0; n.set(n.get().saturating_sub(1)); failing });
        if !took { crate::log::warn("phones: mDNS did not take the pairing announcement; trying again"); }
        *self.phone_announced.lock().unwrap_or_else(|e| e.into_inner()) = port.map(|p| (p, took));
    }

    /// A phone pairing that ended keeps its status for the window, but is no longer announced: decided under the same
    /// lock as a new attempt's announcement, so it never clears the next attempt's port.
    fn phone_tick(&self) {
        let shown = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        let done = shown.as_ref().is_some_and(|h| !matches!(h.status()["state"].as_str(), Some("waiting" | "confirm")));
        #[cfg(test)]
        phone_seam(self);
        let announced = *self.phone_announced.lock().unwrap_or_else(|e| e.into_inner());
        if done && announced.is_some() { self.advertise_phone(&shown, None); }
        // an attempt still shown whose announcement mDNS did not take: again
        if let (false, Some((port, false))) = (done || shown.is_none(), announced) { self.advertise_phone(&shown, Some(port)); }
    }

    /// Show a code to a phone for two minutes: `{attempt, code, expires_at, host_hint, port, qr, state}`; the QR code
    /// says `hm-phone:1?h=<host:port>&c=<code>&f=<this computer's certificate fingerprint>`, and mDNS announces the
    /// port for a phone given the six digits. `host_name` as for computers.
    pub fn phone_pair_start(&self, ctx: &Arc<Ctx>, host_name: Option<&str>) -> Result<Value> {
        let ctx = self.ready(ctx)?;
        let mut shown = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        self.phone_start_in(&mut shown, ctx, host_name)
    }

    fn phone_start_in(&self, shown: &mut std::sync::MutexGuard<'_, Option<pairing::PhoneHost>>, ctx: Arc<Ctx>, host_name: Option<&str>) -> Result<Value> {
        if let Some(old) = shown.take() { old.cancel(); }
        let h = match pairing::start_phone(ctx, self.rt.handle(), self.bind, host_name, self.on_event.clone()) {
            Ok(h) => h,
            Err(e) => { self.advertise_phone(shown, None); return Err(e); }
        };
        let (v, port) = (h.status(), h.port);
        **shown = Some(h);
        self.advertise_phone(shown, Some(port));
        Ok(v)
    }

    /// Tests: start a phone pairing now if nothing holds the pairing shown; its status, or None when it was held.
    #[cfg(test)]
    pub(crate) fn try_phone_pair_start(&self, ctx: &Arc<Ctx>) -> Option<Value> {
        let mut shown = self.phone.try_lock().ok()?;
        self.phone_start_in(&mut shown, ctx.clone(), Some("127.0.0.1")).ok()
    }

    /// The phone pairing shown: `{attempt, state: waiting|confirm|paired|refused|expired|failed, phone?: {name,
    /// platform}, words?, error?, code, expires_at, host_hint, port, qr}`; None without one. Every change is also sent
    /// as `phones-pairing` with this value.
    pub fn phone_pair_status(&self) -> Option<Value> { self.phone.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(|h| h.status()) }

    /// The pairing the owner decides on: the one shown, by the `attempt` its status carried.
    fn shown<'a>(h: &'a Option<pairing::PhoneHost>, attempt: &str) -> Result<&'a pairing::PhoneHost> {
        h.as_ref().filter(|h| h.attempt() == attempt).context("that phone pairing is no longer shown; look at the one shown now")
    }

    /// The owner approved the phone of pairing `attempt`, with `grants` (`{summaries, transcripts, control}`; missing
    /// ones as the defaults: summaries and transcripts on, Control off). Answers the status after.
    pub fn phone_pair_approve(&self, ctx: &Arc<Ctx>, attempt: &str, grants: &Value) -> Result<Value> {
        let g: companion_proto::api::Grants = if grants.is_null() { Default::default() } else {
            let d = companion_proto::api::Grants::default();
            let b = |k: &str, def: bool| -> Result<bool> { match &grants[k] { Value::Null => Ok(def), Value::Bool(b) => Ok(*b), _ => bail!("{k} is on or off") } };
            companion_proto::api::Grants { summaries: b("summaries", d.summaries)?, transcripts: b("transcripts", d.transcripts)?, control: b("control", d.control)? }
        };
        // the port the listener holds now, not the one `ctx` was opened with (0 on a data folder new to it)
        let port = self.listen_port(ctx);
        let h = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        Self::shown(&h, attempt)?.approve(ctx, g, port)
    }

    /// After the owner decided on pairing `attempt`: wait until the phone fetched the decision, at most `within`,
    /// then stop showing the code. True when the phone has it. For a caller that ends once paired (`hrm phones
    /// pair`); the app keeps the attempt to its end instead.
    pub fn phone_pair_end(&self, attempt: &str, within: std::time::Duration) -> Result<bool> {
        let until = std::time::Instant::now() + within;
        let fetched = loop {
            let st = self.phone_pair_status().filter(|s| s["attempt"] == attempt).context("that phone pairing is no longer shown")?;
            if st["fetched"] == true { break true; }
            if std::time::Instant::now() >= until { break false; }
            std::thread::sleep(std::time::Duration::from_millis(100));
        };
        let _ = self.phone_pair_cancel(attempt);
        Ok(fetched)
    }

    pub fn phone_pair_refuse(&self, attempt: &str) -> Result<Value> {
        let h = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        Self::shown(&h, attempt)?.refuse()
    }

    /// Tests: the phone pairing shown, as its host.
    #[cfg(test)]
    pub(crate) fn with_phone_host<T>(&self, f: impl FnOnce(&pairing::PhoneHost) -> T) -> Option<T> { self.phone.lock().unwrap_or_else(|e| e.into_inner()).as_ref().map(f) }

    fn phone_cancel_inner(&self) {
        let mut shown = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(h) = shown.take() { h.cancel(); }
        self.advertise_phone(&shown, None);
    }

    /// Tests: the phone pairing port announced now.
    #[cfg(test)]
    pub(crate) fn phone_announced(&self) -> Option<u16> { self.phone_announced.lock().unwrap_or_else(|e| e.into_inner()).map(|(p, _)| p) }

    /// Tests: the announcement shown was taken by mDNS.
    #[cfg(test)]
    pub(crate) fn phone_registered(&self) -> bool { self.phone_announced.lock().unwrap_or_else(|e| e.into_inner()).is_some_and(|(_, took)| took) }

    /// Tests: what the periodic tick does for the phone pairing.
    #[cfg(test)]
    pub(crate) fn tick_phone(&self) { self.phone_tick(); }

    /// Stop showing the code of pairing `attempt`: answers null, as `phone_pair_status` has nothing to show after.
    pub fn phone_pair_cancel(&self, attempt: &str) -> Result<Value> {
        // compared and taken under one lock: a cancel for an attempt never stops the one that replaced it
        let mut shown = self.phone.lock().unwrap_or_else(|e| e.into_inner());
        Self::shown(&shown, attempt)?;
        #[cfg(test)]
        phone_seam(self);
        if let Some(h) = shown.take() { h.cancel(); }
        self.advertise_phone(&shown, None);
        Ok(Value::Null)
    }
}

/// Whether an announcement is in place: what mDNS said, and with no mDNS running, nothing to announce only on a
/// loopback listener (nobody could reach it); elsewhere its absence is a failure to try again.
pub(crate) fn registration_took(registered: Option<bool>, loopback: bool) -> bool { registered.unwrap_or(loopback) }

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

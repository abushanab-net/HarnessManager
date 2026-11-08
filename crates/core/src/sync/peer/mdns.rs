//! Finding paired computers on the same network: `_hrm._tcp.local.`, TXT `id`, `name`, `port`, `pairing`
//! (1 while a code is shown) and `pairing_port`. What the browse finds is kept by id for the pulls.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::Mutex;

use anyhow::Result;
use mdns_sd::{ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo};

const TY: &str = "_hrm._tcp.local.";

#[derive(Clone, Debug)]
pub struct Found {
    pub ip: IpAddr,
    pub port: u16,
    pub name: String,
    pub pairing_port: Option<u16>,
}

static FOUND: Mutex<BTreeMap<String, Found>> = Mutex::new(BTreeMap::new());

pub struct Mdns {
    daemon: ServiceDaemon,
    fullname: String,
    id: String,
    name: String,
    port: u16,
}

fn note(me: &str, r: &ResolvedService) {
    let txt = |k: &str| r.txt_properties.get_property_val_str(k).unwrap_or("").to_string();
    let id = txt("id");
    if id.is_empty() || id == me { return; }
    // an IPv4 address first: the VPN and most home networks route it
    let Some(ip) = r.addresses.iter().map(|a| a.to_ip_addr()).min_by_key(|a| a.is_ipv6()) else { return };
    let f = Found { ip, port: txt("port").parse().unwrap_or(r.port), name: txt("name"), pairing_port: (txt("pairing") == "1").then(|| txt("pairing_port").parse().ok()).flatten() };
    FOUND.lock().unwrap_or_else(|e| e.into_inner()).insert(id, f);
}

impl Mdns {
    /// Advertise this computer and browse for the others.
    pub fn start(id: &str, name: &str, port: u16) -> Result<Mdns> {
        let daemon = ServiceDaemon::new()?;
        let rx = daemon.browse(TY)?;
        let me = id.to_string();
        std::thread::spawn(move || {
            while let Ok(ev) = rx.recv() {
                match ev {
                    ServiceEvent::ServiceResolved(r) => note(&me, &r),
                    ServiceEvent::ServiceRemoved(_, fullname) => { FOUND.lock().unwrap_or_else(|e| e.into_inner()).retain(|id, _| !fullname.starts_with(id.as_str())); }
                    _ => {}
                }
            }
        });
        let mut m = Mdns { daemon, fullname: String::new(), id: id.into(), name: name.into(), port };
        m.advertise(None)?;
        Ok(m)
    }

    /// (Re)register with the pairing flag and port.
    pub fn advertise(&mut self, pairing_port: Option<u16>) -> Result<()> {
        if !self.fullname.is_empty() { let _ = self.daemon.unregister(&self.fullname); }
        let host: String = crate::util::hostname().chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
        let props = [("id", self.id.clone()), ("name", self.name.clone()), ("port", self.port.to_string()),
            ("pairing", if pairing_port.is_some() { "1" } else { "0" }.to_string()), ("pairing_port", pairing_port.map(|p| p.to_string()).unwrap_or_default())];
        let info = ServiceInfo::new(TY, &self.id, &format!("{host}.local."), "", self.port, &props[..])?.enable_addr_auto();
        self.fullname = info.get_fullname().to_string();
        self.daemon.register(info)?;
        Ok(())
    }

    pub fn stop(&self) {
        if !self.fullname.is_empty() { let _ = self.daemon.unregister(&self.fullname); }
        let _ = self.daemon.shutdown();
    }
}

/// Where a computer answered from, by id.
pub fn lookup(id: &str) -> Option<(IpAddr, u16)> { FOUND.lock().unwrap_or_else(|e| e.into_inner()).get(id).map(|f| (f.ip, f.port)) }

/// A computer showing a pairing code: (address, pairing port, id).
pub fn pairing_host() -> Option<(IpAddr, u16, String)> {
    FOUND.lock().unwrap_or_else(|e| e.into_inner()).iter().find_map(|(id, f)| f.pairing_port.map(|p| (f.ip, p, id.clone())))
}

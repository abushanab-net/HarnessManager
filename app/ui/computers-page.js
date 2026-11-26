// Computers, as the 1.10 drawing draws it (docs/project-review-2026-11/mockups/a-computers.html): the head with the one
// primary "Pair a computer…", a card per computer (this one first: its dot and name, a ⋯ menu, four facts and the
// sessions running there), the phones paired with this computer (S24: their grants as switches, Revoke…), then "How
// your computers sync": the two ways side by side and three switches. The chip, the helpers and the dialogs (pairing a
// computer or a phone, the hub, the recovery key, the first-run choice) are in computers.js.

import { S, call, view, toast, attempt, mutate, changed, openDialog, closeDialog, dlgHead, confirmDialog, action, render, pickFolder, loadState, patch, dialogOpen, keyStore, openMenu } from './core.js';
import { esc, dur } from './lib.js';
import { computersStatus, syncOn, computers, colorIdx, heardOn, transportLine, nameOf, WAYS, GRANTS, PLATFORM, inviteDialog, ringEnterDialog, ringShowAgain, hubLeave } from './computers.js';
import { info, row, toggle } from './settings.js';
import { pageHead, plural, pill } from './parts.js';

const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');
/** When a computer was last seen, in its largest unit: "1 m ago", "3 h ago", "2 d ago". */
const since = (ms) => { const m = Math.max(1, Math.round(ms / 60000)); return (m >= 1440 ? `${Math.floor(m / 1440)} d` : m >= 60 ? `${Math.floor(m / 60)} h` : `${m} m`) + ' ago'; };
const OS = { windows: 'Windows', macos: 'macOS', linux: 'Linux' };
/** The one line of a card whose computer's journal did not arrive whole (`gap` of the status). */
const GAP = "Part of this computer's history is missing here.";
const dots = (label, act, data, testid) => `<button class="btn sm icon" data-act="${act}" ${data} aria-haspopup="menu" aria-label="${esc(label)}" title="${esc(label)}" data-testid="${testid}">⋯</button>`;

view('computers', async (sheet) => {
  sheet.className = 'sheet';
  const st = await loadState();
  const c = await computersStatus();
  const ts = syncOn(c) ? await call('transcripts_status').catch(() => null) : null;
  // the phones, once this app has them (an older core answers with an error: no card then)
  const phones = await call('phones_list').catch(() => null);
  // the status: how many computers share their sessions and through what, and which are connected directly
  const n = computers(c).length, direct = c.peers.filter((p) => p.connected).map((p) => p.name);
  const status = !syncOn(c) ? 'Sync is off: only this computer.'
    : `${plural(n, 'computer shares its', 'computers share their')} sessions ${c.mode === 'hub' ? 'through a hub' : c.mode === 'folder' ? 'through a folder' : 'directly'}${direct.length && c.mode !== 'off' ? `; ${esc(direct.join(', '))} ${direct.length === 1 ? 'is' : 'are'} connected directly` : ''}.`;
  patch(sheet, `<div class="page-body b-page" data-b-page="computers">
    ${pageHead('computers', status, `${phones ? '<button class="btn" data-act="phone-pair" data-testid="phone-pair-head">Pair a phone…</button>' : ''}<button class="btn primary" data-act="pair" data-testid="pair-head">Pair a computer…</button>`)}
    ${c.error ? `<div role="alert" class="notice-bar warn" data-testid="sync-error">${esc(c.error)}</div>` : ''}
    <div class="pcs" data-testid="computer-cards">${computers(c).map((d) => card(c, d, st)).join('')}</div>
    ${phones ? phonesCard(phones) : ''}
    ${syncCard(c, st.settings, ts)}
  </div>`);
});

// ---------- a card per computer ----------
/** How a computer is reached, in a few words, and its tone: [text, class]. Details… in the card's ⋯ has the exact line. */
function connection(c, d, me, now) {
  if (me) return [c.mode === 'hub' ? 'Through the hub' : c.mode === 'folder' ? 'Through the folder' : syncOn(c) ? 'Direct connection only' : 'Sync is off', ''];
  if (d.connected) return ['Direct connection', 'ok'];
  if (d.path === 'relay' && d.relayed_by) return [`Relayed through ${nameOf(c, d.relayed_by)}`, ''];
  const via = d.transport === 'hub' ? 'hub' : 'folder';
  return d.direct && !heardOn(d, now) ? [`Not reachable; ${via} only`, 'warnT'] : [`Through the ${via}`, ''];
}
function card(c, d, st) {
  const me = d === c.this, now = Date.now(), id = d.id || 'this', running = (d.running || []).length;
  const seen = me || d.connected || (d.last_seen && now - d.last_seen < 60000) ? 'Now' : d.last_seen ? since(now - d.last_seen) : 'Not yet';
  const [conn, tone] = connection(c, d, me, now);
  const menu = dots(me ? 'More actions: Rename…' : `More actions for ${d.name}: Forget…`, 'pc-menu', `data-id="${esc(d.id || '')}" data-name="${esc(d.name)}" data-me="${me ? 1 : ''}" data-direct="${d.direct ? 1 : ''}" data-address="${esc(d.address || '')}"`, `pc-menu-${esc(id)}`);
  return `<section class="card panel-card pc" data-key="computer-${esc(id)}" data-testid="computer-${esc(id)}">
    <div class="pc-head"><i class="pc-dot c${colorIdx(c, d.id)}"></i><h2 class="section trunc" title="${esc(d.name)}">${esc(d.name)}</h2>${me ? pill('This computer', 'ok') : ''}<span class="grow"></span>${!me && d.direct ? `<button class="btn sm" data-act="computers-now" data-testid="now-${esc(id)}">Sync now</button>` : ''}${menu}</div>
    <div class="pc-body">
      <dl class="kv"><dt>System</dt><dd>${esc(OS[me ? st.os : d.os] || (me ? st.os : d.os) || '—')}</dd><dt>App version</dt><dd>${esc((me ? st.version : d.app_version) || '—')}</dd><dt>Last seen</dt><dd>${esc(seen)}</dd>
        <dt>Connection</dt><dd class="trunc ${tone}" title="${esc(conn)}" data-testid="transport">${esc(conn)}</dd></dl>
      <div class="count"><b class="key ${running ? '' : 'dim'}">${running}</b><span>${running === 1 ? 'session' : 'sessions'}<br>running</span></div>
    </div>${!me && d.gap ? `<p class="pc-note warnT" data-testid="gap-${esc(id)}">${esc(GAP)}</p>` : ''}</section>`;
}
// the ⋯ of a card: Rename… for this computer; Sync now, Edit address… (a paired one) and Forget… for another; Details… for both
action('pc-menu', (el) => {
  const { id, name, me, direct, address } = el.dataset, on = syncOn(S.cache.computers), details = { label: 'Details…', testid: 'pc-details', run: () => detailsDialog(id) };
  openMenu(el, me ? [
    { label: 'Rename…', testid: 'pc-rename', run: renameDialog },
    ...(on ? [{ label: 'Sync now', testid: 'computers-now', run: syncNow }] : []),
    details,
  ] : [
    { label: 'Sync now', testid: 'computers-now', run: syncNow },
    ...(direct ? [{ label: 'Edit address…', testid: `addr-edit-${id}`, run: () => addressDialog(id, name, address) }] : []),
    details,
    { label: 'Forget…', testid: `forget-${id}`, danger: true, run: () => forget(id, name) },
  ]);
});
/** What the four facts of a card leave out: the journal's progress, what the computer holds, and for another computer
 *  how it is reached now, when it was paired and last pulled, its address. */
function detailsDialog(id) {
  const c = S.cache.computers, d = computers(c).find((x) => (x.id || '') === id), me = d === c.this, seq = d.last_seq || 0, done = me ? (d.published_seq ?? seq) : (d.applied_seq || 0);
  const lines = [me ? '' : `<b>Reached</b> ${esc(transportLine(c, d))}`,
    me ? '' : d.direct ? `<b>Paired</b> ${d.paired_at ? esc(stamp(d.paired_at)) : 'yes'}` : '<b>Not paired</b>: known through the folder or the hub only',
    me ? '' : `<b>Last pull</b> ${d.last_pull ? esc(ago(d.last_pull)) : 'never'}`,
    d.address ? `<b>Address</b> ${esc(d.address)}` : '', d.relayed_by ? `<b>Relayed through</b> ${esc(nameOf(c, d.relayed_by))}` : '',
    `<b>Journal</b> ${done.toLocaleString()} of ${seq.toLocaleString()} entries ${me ? 'published' : 'applied here'}`,
    d.projects != null || d.accounts != null ? `<b>Holds</b> ${[d.projects != null ? plural(d.projects, 'project') : '', d.accounts != null ? plural(d.accounts, 'account') : ''].filter(Boolean).join(' · ')}` : ''];
  openDialog(`${dlgHead(d.name)}<div class="dlg-text" data-testid="pc-details-body">${lines.filter(Boolean).join('<br>')}</div>
    <div class="modal-foot"><span class="acts"><button class="btn-primary" data-act="dialog-close">Close</button></span></div>`, { size: 'sm', label: 'Details of ' + d.name });
}
/** A dialog with one field: Save (or Enter) hands over what was typed, trimmed; `save` answers false to keep the dialog open. */
function ask(title, id, label, value, extra, hint, ok, save) {
  openDialog(`${dlgHead(title)}<div class="form-field"><label for="${esc(id)}">${label}</label><input id="${esc(id)}" value="${esc(value)}" ${extra} data-testid="${esc(id)}"><span class="muted">${hint}</span></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="ask-ok" data-testid="${esc(id)}-save">${ok}</button></span></div>`, { size: 'sm', label: title });
  const inp = document.getElementById(id), go = document.getElementById('ask-ok');
  go.onclick = () => { if (save(inp.value.trim()) !== false) closeDialog(); };
  inp.addEventListener('keydown', (e) => { if (e.key === 'Enter') go.click(); });
}
const renameDialog = () => ask('Rename this computer', 'st-devname', 'Name', S.cache.computers.this.name, 'maxlength="40"',
  'The name the other computers show for this one, in their lists, on sessions and on the chip at the top. The default is the computer name Windows or macOS gives it.', 'Rename',
  (v) => { if (!v) { toast('Enter a computer name.', true); return false; } mutate(() => call('computers_set', { device_name: v }), 'Saved'); });
const addressDialog = (id, name, address) => ask('Address of ' + name, 'addr-' + id, 'Address', address, 'class="mono" placeholder="name:port" maxlength="120"',
  `Where to reach ${esc(name)} when it is not on this network: a VPN name, with the port. Empty leaves it to this network.`, 'Save',
  (v) => { mutate(() => call('computers_set_peer', { id, address: v }), 'Saved'); });
async function forget(id, name) {
  if (await confirmDialog('Forget ' + name, `Forget <b>${esc(name)}</b>? This computer stops connecting to it and drops its certificate; what was merged from it stays. Pair again to reconnect.`, 'Forget')) mutate(() => call('computers_forget', { id }), 'Forgotten');
}
async function syncNow() {
  const r = await attempt(() => call('computers_now'));
  if (!r) return;
  const n = (r.peers || []).length;
  toast(r.error ? 'Sync round: ' + r.error : `Synced with ${n} other ${n === 1 ? 'computer' : 'computers'}`, !!r.error);
  changed();
}
action('computers-now', syncNow);

// ---------- the phones: what each may do, as switches saved at once, and Revoke… ----------
const ROUTE = { direct: 'direct', hub: 'through your hub' };
function phonesCard(list) {
  // the revoked phones of the last week stay, with their acknowledgements as they stand now (the cut may come later)
  const now = Date.now(), on = list.filter((p) => !p.revoked_at), gone = list.filter((p) => p.revoked_at && now - p.revoked_at < 7 * 86400e3);
  const line = (p) => [esc(PLATFORM[p.platform] || p.platform || 'Phone'), esc(ROUTE[p.route] || 'not connected'),
    `last seen ${p.last_seen ? (now - p.last_seen < 60000 ? 'now' : since(now - p.last_seen)) : 'never'}`, p.paired_at ? `paired ${esc(new Date(p.paired_at).toLocaleDateString([], { day: 'numeric', month: 'short' }))}` : ''].filter(Boolean).join(' · ');
  const grant = (p, [k, t]) => `<span class="ph-sw">${t}${toggle(k, !!(p.grants || {})[k], `${t} for ${p.name}`, `phone-${esc(p.phone_id)}-${k}`, 'phone-grant')}</span>`;
  return `<section class="card panel-card set-pane" data-testid="phones">
    <div class="pane-head"><h2 class="section">Phones</h2><span class="muted" data-testid="phones-count">${on.length}</span>${info('phones', `<b>Summaries</b>: limits, sessions and usage. <b>Transcripts</b>: what each session said and did. <b>Control</b>: answer requests, reply and Stop; off unless you turn it on.<br>A change reaches the phone at its next request.`)}<span class="grow"></span></div>
    ${on.map((p) => `<div class="srow ph-row" data-key="phone-${esc(p.phone_id)}" data-testid="phone-${esc(p.phone_id)}">
      <div class="what"><div class="row"><b class="trunc" title="${esc(p.name)}">${esc(p.name)}</b></div><p data-testid="phone-line">${line(p)}</p></div>
      <div class="ctl" data-id="${esc(p.phone_id)}">${GRANTS.map((g) => grant(p, g)).join('')}<button class="btn sm negT" data-act="phone-revoke" data-id="${esc(p.phone_id)}" data-name="${esc(p.name)}" data-testid="phone-revoke-${esc(p.phone_id)}">Revoke…</button></div></div>`).join('')
      || '<p class="set-none" data-testid="phones-none">No phone yet. Pair a phone… shows a code to scan with Harness Manager on the phone.</p>'}
    ${gone.map((p) => `<div class="srow ph-row gone" data-testid="phone-${esc(p.phone_id)}"><div class="what"><div class="row"><b class="trunc muted" title="${esc(p.name)}">${esc(p.name)}</b></div>
      <p data-testid="phone-line">Revoked ${esc(new Date(p.revoked_at).toLocaleDateString([], { day: 'numeric', month: 'short' }))} · ${ACK.map(([k, , w]) => `${w} ${mark((p.acks || {})[k])}`).join(' · ')}</p></div></div>`).join('')}
    <p class="pc-note">A phone is never a full peer: it gets no recovery key and no admin token. Revoking confirms on this computer, the hub and the push relay.</p>
  </section>`;
}
// a grant switch: the phone's three grants saved together; put back when the save fails
action('phone-grant', async (el) => {
  const box = el.closest('.ctl'), grants = Object.fromEntries([...box.querySelectorAll('[data-act=phone-grant]')].map((i) => [i.dataset.key, i.checked]));
  if (await mutate(() => call('phones_set_grants', { id: box.dataset.id, grants }), 'Saved') === undefined) el.checked = !el.checked;
});
/** What came back from a revoke: true confirmed, false not confirmed, null not used by this phone (yet). */
const ACK = [['connection', 'Its open connections cut', 'connection'], ['hub', 'Its hub token and mailbox deleted', 'hub'], ['push', 'Its push handle removed from the relay', 'push']];
const mark = (v) => (v === true ? '<span class="ok">confirmed</span>' : v === false ? '<span class="negT">not confirmed</span>' : '<span class="muted">not used yet</span>');
action('phone-revoke', async (el) => {
  const { id, name } = el.dataset;
  if (!(await confirmDialog('Revoke ' + name, `Revoke <b>${esc(name)}</b>? It loses access to this computer at once: its connections are cut, its hub token and mailbox go, and the push relay forgets it. Pair it again to give it access.`, 'Revoke'))) return;
  const r = await mutate(() => call('phones_revoke', { id }));
  if (!r) return;
  const a = r.acks || {};
  openDialog(`${dlgHead(name + ' is revoked')}<div class="dlg-text" data-testid="phone-acks">${ACK.map(([k, t]) => `<div data-testid="phone-ack-${k}"><b>${t}</b>: ${mark(a[k])}</div>`).join('')}</div>
    <div class="modal-foot"><span class="acts"><button class="btn-primary" data-act="dialog-close">Close</button></span></div>`, { size: 'sm', label: name + ' is revoked' });
});

// ---------- how the computers sync: the two ways, one in use at most, and what travels ----------
function syncCard(c, s, ts) {
  const hub = c.hub || {}, folderOn = c.mode === 'folder', hubOn = c.mode === 'hub' && !!hub.joined;
  const way = (testid, on, name, tip, state, line, acts) => `<div class="way ${on ? 'on' : ''}" data-testid="${testid}"><span class="radio"></span><div class="two"><div class="row"><b>${name}</b>${state}${tip}</div>${line}</div>${acts}</div>`;
  const inUse = pill('In use', 'ok');
  return `<section class="card panel-card set-pane" data-testid="computers">
    <div class="pane-head"><h2 class="section">How your computers sync</h2>${info('syncmode', 'Pick one per computer. Each computer writes its own journal there and reads the others\'; keys and logins never go in it. Switching later keeps everything, because the journal is the same either way.<br><b>Default: off.</b>')}
      ${syncOn(c) ? `<span class="small muted" data-testid="last-round">${c.last_run ? 'Last round ' + esc(ago(c.last_run)) : 'No round yet'}</span>` : ''}<button class="btn sm quiet" data-act="firstrun-open" data-testid="firstrun-open">First-run choices…</button></div>
    <div class="ways">
      ${way('sync-folder', folderOn, 'A shared folder', info('way-folder', WAYS.folder), folderOn ? inUse : '',
        c.folder ? `<div class="mono trunc" title="${esc(c.folder)}" data-testid="sync-folder-path">${esc(c.folder)}</div>` : '<div class="muted" data-testid="sync-folder-path">No folder chosen</div>',
        `<button class="btn sm" data-act="sync-folder-pick" data-testid="sync-folder-pick">${c.folder ? 'Change…' : 'Choose…'}</button>${folderOn ? dots('More actions: Stop syncing…', 'way-menu', 'data-way="folder"', 'sync-folder-menu') : ''}`)}
      ${way('sync-hub', hubOn, 'A hub you host', info('way-hub', `${WAYS.hub}<br><b>The recovery key</b> is the ring key that seals every chunk, presence report and transcript before it goes to the hub, so the hub stores what it cannot read. It is made on the first computer that joins the hub, kept in ${esc(keyStore())}, carried to paired computers with the pairing, and shown once: 8 groups of 4 letters and digits for your password manager. A computer that is not paired needs it typed in.`),
        hubOn ? (hub.ring ? inUse : `<span class="pill bad" title="This computer cannot read or write the hub. Enter the recovery key shown on the computer that first joined, or pair with it." data-testid="no-ring">No recovery key</span>`) : '',
        hubOn ? `<div class="row"><span class="mono trunc" title="${esc(hub.url || '')}" data-testid="hub-url">${esc(hub.url || '')}</span><span class="muted" data-testid="hub-line">· ${plural(hub.devices || 0, 'computer')} · ${hub.ws ? 'live' : 'polling'}</span></div>` : '<div class="muted" data-testid="hub-url">Not joined</div>',
        `${!hubOn ? '<button class="btn sm" data-act="hub-join" data-testid="hub-join">Join with a code…</button>' : hub.ring ? '<button class="btn sm" data-act="hub-invite" data-testid="hub-invite">Invite a computer…</button>' : '<button class="btn sm" data-act="ring-enter" data-testid="ring-enter">Enter the recovery key…</button>'}${hubOn || hub.ring ? dots('More actions: the recovery key, Leave…', 'way-menu', `data-way="hub" data-on="${hubOn ? 1 : ''}" data-ring="${hub.ring ? 1 : ''}"`, 'sync-hub-menu') : ''}`)}
    </div>
    ${row('transcripts', 'Carry transcripts', info('transcripts', `Also share this computer's session transcripts, so they can be read and resumed on another computer. A transcript over 20 MB is not carried; the folder holds up to 200 MB of this computer's transcripts, the oldest going first.${ts ? `<br>Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB${ts.skipped && ts.skipped.length ? ` · ${ts.skipped.length} skipped` : ''}` : ''}<br><b>Default: on.</b>`), 'Over 20 MB is skipped; the store holds 200 MB.', toggle('sync_transcripts', s.sync_transcripts !== false, 'Carry transcripts', 'st-transcripts', 'computers-set'))}
    ${row('presence', 'Share what runs here', info('presence', 'Writes what is running here every minute while the app is open, so the other computers show it in Running now.<br><b>Default: on.</b>'), "Your other computers see this one's sessions, every minute.", toggle('sync_presence', s.sync_presence !== false, 'Share what runs here', 'st-presence', 'computers-set'))}
    ${row('direct', 'Connect directly', info('direct', 'Finds your other computers on this network (mDNS) or at an address you give (a VPN name), after pairing once with a six-digit code, and talks to them over TLS: what runs there every ten seconds, their history the moment it is written. The folder keeps working underneath; a computer reached neither way is still merged through it.<br><b>Default: off.</b>'), 'Paired computers talk over TLS; the folder keeps working.',
      `${toggle('direct_connection', s.direct_connection, 'Connect directly', 'st-direct', 'computers-set')}<button class="btn sm" data-act="listen-port" title="The port the direct connection listens on" data-testid="listen-port-change">Listen port <span data-testid="listen-port">${c.port || 'chosen at first use'}</span></button>`)}
  </section>`;
}
// the ⋯ of a way: Stop syncing… for the folder; the recovery key, Invite and Leave… for the hub
action('way-menu', (el) => {
  const { way, on, ring } = el.dataset;
  openMenu(el, way === 'folder' ? [{ label: 'Stop syncing…', testid: 'sync-off', danger: true, run: syncOff }] : [
    ...(on && !ring ? [{ label: 'Invite a computer…', testid: 'hub-invite', run: inviteDialog }] : []),
    ...(ring ? [{ label: 'Show the recovery key…', testid: 'ring-show', run: ringShowAgain }, { label: 'Enter the recovery key…', testid: 'ring-enter', run: ringEnterDialog }] : []),
    ...(on ? [{ label: 'Leave the hub…', testid: 'hub-leave', danger: true, run: hubLeave }] : []),
  ]);
});
// a switch: saved through computers_set, which answers with the new status
action('computers-set', async (el) => {
  if (await mutate(() => call('computers_set', { [el.dataset.key]: el.checked }), 'Saved') === undefined) el.checked = !el.checked;
});
action('sync-folder-pick', async () => {
  const p = await pickFolder();
  if (p && await mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p) !== undefined) (await import('./restore.js')).welcomeBack(p);
});
async function syncOff() { if (await confirmDialog('Stop syncing', 'Leave the folder? Merged history stays on this computer, and the folder keeps what was written.', 'Leave')) mutate(() => call('computers_set', { sync_mode: 'off' }), 'Sync is off; the folder keeps what was written'); }
// the port the direct connection listens on
action('listen-port', () => {
  openDialog(`${dlgHead('Listen port')}<div class="form-field"><label for="lp-v">Port</label><input class="mono" id="lp-v" type="number" min="0" max="65535" value="${(S.cache.computers || {}).port || 0}" data-testid="lp-v"><span class="muted">0 picks a free port once and keeps it. Your other computers find this one by mDNS or by its address with this port.</span></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="lp-ok" data-testid="lp-ok">Save</button></span></div>`, { size: 'sm', label: 'Listen port' });
  document.getElementById('lp-ok').onclick = async () => {
    const v = +document.getElementById('lp-v').value;
    if (!(v >= 0 && v <= 65535)) return toast('A port is 0 to 65535', true);
    closeDialog();
    mutate(() => call('computers_set', { listen_port: v }), 'Saved');
  };
});
// presence arrives every 10 s while a peer is connected: the page follows in place (the reconciler keeps what is being edited)
document.addEventListener('presence', () => { if (S.route.view === 'computers' && S.state && !dialogOpen()) render(true); });

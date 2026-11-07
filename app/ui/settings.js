// Settings, as the reference draws it: the finding, a sticky row of section chips, and every section a folding card
// ("N settings") of rows: a title with its ⓘ (the explanation and the default), a one-line description, the control.
// Switches and chips save at once; typed fields have a button beside them. Computers (the two sync cards, the rows, a
// card per computer) and Backups and archive are sections of the same page. Harness Manager's own update lives here.

import { S, call, view, toast, attempt, mutate, changed, openDialog, closeDialog, dlgHead, confirmDialog, action, render, pickFolder, saveFile, pickFile, applyTheme, patch, dialogOpen } from './core.js';
import { resetGuides } from './guide.js';
import { esc, when, dur } from './lib.js';
import { winTrack } from './accounts.js';
import { computersStatus, syncOn, computers, colorIdx, heardOn, transportLine, nameOf, syncChoices } from './computers.js';

// ---------- the parts ----------
/** An ⓘ that explains a setting on hover or keyboard focus. */
const info = (id, html) => `<span class="b-tip"><button type="button" class="icon-btn" aria-label="About this setting" aria-describedby="tip-${id}" data-testid="tip-${id}">i</button><span role="tooltip" id="tip-${id}">${html}</span></span>`;
/** A row: the title with its ⓘ, the description, the control. */
const row = (key, title, tip, text, ctl) => `<div class="b-setting" data-key="setting-${key}"><div><div class="b-name"><b>${title}</b>${tip || ''}</div><div class="muted">${text}</div></div><div class="b-setting-control">${ctl}</div></div>`;
// a switch that saves as soon as it is flipped, and a chip group that does the same
const sw = (key, on, label, testid, act = 'set-now') => `<label class="b-switch"><input type="checkbox" data-act="${act}" data-key="${key}" ${on ? 'checked' : ''} data-testid="${testid}"> ${label}</label>`;
const chips = (key, cur, opts, testid) => `<div class="chip-group" role="group" aria-label="${esc(key.replaceAll('_', ' '))}" data-testid="${testid}">${opts.map(([v, l]) => `<button class="filter-chip ${String(cur) === String(v) ? 'active' : ''}" aria-pressed="${String(cur) === String(v)}" data-act="set-now" data-key="${key}" data-v="${v}">${l}</button>`).join('')}</div>`;
const field = (id, label, extra = '') => `<div class="form-field"><label for="${id}">${label}</label><input id="${id}" ${extra}></div>`;
const btn = (label, act, extra = '') => `<button class="btn-secondary" data-act="${act}" ${extra}>${label}</button>`;
const slug = (title) => title.toLowerCase().replace(/[^a-z]+/g, '-');
const NAV = [['appearance', 'Appearance'], ['window', 'Window'], ['updates', 'Updates'], ['usage', 'Limits'], ['connections', 'Connections'], ['log', 'Log'], ['maintenance', 'Maintenance'], ['computers', 'Computers'], ['backups', 'Backups']];
/** A section card that folds; its rows stay on the page while folded. */
function section(id, title, body) {
  const n = (body.match(/class="b-setting"/g) || []).length;
  return `<details class="panel-card b-section" id="sec-${id}" data-testid="sec-${slug(title)}" ${shut.has(id) ? '' : 'open'}><summary data-act="set-fold" data-v="${id}"><span class="panel-title">${title}</span><span class="muted">${n} ${n === 1 ? 'setting' : 'settings'}</span></summary><div class="b-section-body">${body}</div></details>`;
}
const shut = new Set();
action('set-fold', (el) => {
  const d = el.parentElement;
  d.open = !d.open;
  if (d.open) shut.delete(el.dataset.v); else shut.add(el.dataset.v);
});
const kb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');

view('settings', async (sheet, params) => {
  sheet.className = 'sheet';
  const st = await call('state');
  S.state = st;
  applyTheme(st);
  const s = st.settings;
  const c = await computersStatus().catch(() => null);
  const [ts, b, ar] = await Promise.all([c && syncOn(c) ? call('transcripts_status').catch(() => null) : null, call('backups').catch(() => null), call('archive_status').catch(() => null)]);
  // the sign-in entry and the choice of terminal are Windows things; macOS opens Terminal
  const win = !st.os || st.os === 'windows';
  patch(sheet, `<div class="page-body b-page" data-b-page="settings">
    <header class="page-heading"><div><h1>Settings stay on this computer.</h1><div class="heading-facts">Saved as you change them · keys and logins never enter these files or backups</div></div></header>
    <nav class="b-section-nav chip-group" aria-label="Settings sections">${NAV.map(([id, label]) => `<button class="filter-chip" data-act="set-jump" data-v="${id}">${label}</button>`).join('')}</nav>
    ${section('appearance', 'Appearance',
      row('brand', 'Theme', info('brand', '<b>Harness (the default):</b> porcelain and night, Inter.<br><b>VektraGrid:</b> ivory and deep green, Garamond headings; deep teal leads, plum marks the router, sand gold is the warning.'), 'The look of this window; light and dark follow the choice below.', chips('brand', s.brand || 'hm', [['hm', 'Harness'], ['vg', 'VektraGrid']], 'st-brand'))
      + row('theme', 'Light and dark', info('theme', '<b>System (the default):</b> follows the light or dark setting of Windows or macOS.<br><b>Light</b> or <b>Dark</b>: always that.'), 'Follow the system, or keep one.', chips('theme', s.theme, [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']], 'st-theme'))
      + row('quota', 'Limits shown as', info('quota', '<b>Left (the default):</b> how much of each limit you still have, as the tools show it.<br><b>Used:</b> how much has gone.'), 'Applies to every ring and bar.', chips('quota_display', s.quota_display, [['left', 'Left'], ['used', 'Used']], 'st-quota'))
      + row('motion', 'Moving routes', info('motion', 'Pulses along the live routes, as fast as their traffic. Off, or with reduced motion in Windows or macOS, the routes are a still picture.<br><b>Default: on.</b>'), "The live routes on the Overview. Off, or the system's reduced motion, keeps them still.", sw('moving_routes', s.moving_routes, 'Animate the routes', 'st-motion')))}
    ${section('window', win ? 'Window and terminal' : 'Window',
      row('tray', 'Close to the tray', info('tray', '<b>On (the default):</b> the window\'s X hides Harness Manager to the tray; usage and limits keep refreshing, and the tray icon opens it again. Quit from the tray menu.<br><b>Off:</b> the X quits.'), 'The window hides and the app keeps refreshing until you choose Quit.', sw('close_to_tray', s.close_to_tray, 'Keep running in the tray', 'st-tray'))
      + (win ? row('auto', 'Launch at sign-in', info('auto', 'Starts Harness Manager in the tray when you sign in to Windows, so usage and limits are current when you open it.<br><b>Default: off.</b>'), 'Starts hidden in the tray.', `<label class="b-switch"><input type="checkbox" data-act="autostart" ${st.autostart ? 'checked' : ''} data-testid="st-autostart"> Start with Windows</label>`) : '')
      + (win ? row('term', 'Terminal', info('term', '<b>Windows Terminal</b> (the default): Start and Resume open a new Windows Terminal window running the account.<br><b>Console window</b>: the classic console, for computers without Windows Terminal or if you prefer it.'), 'Where Start and Resume open a console.', chips('terminal', s.terminal, [['wt', 'Windows Terminal'], ['console', 'Console']], 'st-term')) : ''))}
    ${section('updates', 'Updates',
      row('self', 'Harness Manager', info('self', 'New versions are looked for at launch and every hour; only signed updates are installed.<br><b>Default: checks at launch and every hour.</b>'), selfUpd && selfUpd.version ? `Version <b>${esc(selfUpd.version)}</b> is available; you have ${esc(st.version)}.${selfUpd.notes ? ' ' + esc(selfUpd.notes) : ''} Signed updates only.` : `Version ${esc(st.version)}${selfUpd ? ' · the newest' : ''}. Checks at launch and every hour · signed updates only.`,
        selfUpd && selfUpd.version ? '<button class="btn-primary" data-act="self-install" data-testid="self-install">Update and restart…</button>' : btn('Check for updates', 'self-check', 'data-testid="self-check"'))
      + row('upd', 'New versions of your tools', info('upd', 'Asks each vendor for the newest Claude Code, Codex, OpenCode and Antigravity CLI when Harness Manager starts; Apps in the sidebar shows how many have one.<br><b>Default: off.</b> Apps &gt; Check for updates looks at any time.'), 'Ask vendors for new versions when the app starts.', sw('check_updates_at_launch', s.check_updates_at_launch, 'Check at launch', 'st-updcheck')))}
    ${section('usage', 'Usage and limits',
      row('refresh', 'Refresh', info('refresh', 'How often, in minutes, Harness Manager reads new usage from the tools\' logs and the cached limits, in the background.<br><b>Default: 5.</b> A smaller number updates sooner but reads the logs more often. The ↻ at the top reads them at once whatever this is.'), 'Read usage and cached limits in the background.', chips('refresh_minutes', String(s.refresh_minutes), [['1', '1 min'], ['5', '5 min'], ['15', '15 min']], 'st-refresh'))
      + row('live', 'Live Claude limits', info('live', '<b>Off (the default):</b> limits are what Claude Code last saved, so they are as fresh as the account\'s last run; a window whose reset time has passed shows as empty.<br><b>On:</b> when those numbers are more than 10 minutes old, Harness Manager asks Anthropic for current ones with the login Claude Code saved, while it is valid. It never refreshes or copies that login.'), 'Read current limits with the saved login; never starts a session or reads conversations.', `<label class="b-switch"><input type="checkbox" id="st-live" data-act="live-now" ${st.live_quota ? 'checked' : ''} data-testid="st-live"> Ask Anthropic for current limits</label>`))}
    ${section('connections', 'Connections',
      row('port', 'Router port', info('port', 'The port of the local router that routed accounts talk to. It listens on 127.0.0.1 only, so nothing outside this computer can reach it.<br><b>Default: 15731.</b> Change it only if another program uses that port; routed accounts use the new port the next time they start.'), '127.0.0.1 only.', `${field('st-port', 'Port', `class="mono" type="number" min="1" max="65535" value="${s.router_port}"`)}${btn('Change', 'set-field', 'data-key="router_port" data-from="st-port" data-testid="st-port-save"')}`)
      + row('reg', 'Account list', info('reg', 'Where profiles.json lives: your accounts, providers, routes, MCP servers, skills and prompts. No keys or logins are ever stored in it.<br><b>Empty (the default):</b> it stays in this computer\'s Harness Manager folder (%APPDATA%\\HarnessManager). Enter a path in a OneDrive folder to share the same accounts with your other computer; each computer keeps its own keys and logins.'), 'Share profiles through a folder; each computer keeps its own keys and logins.', `${field('st-reg', 'profiles.json path', `class="mono wide" value="${esc(s.registry_path)}" placeholder="${esc(st.registry_path)}"`)}${btn('Move…', 'set-field', 'data-key="registry_path" data-from="st-reg" data-testid="st-reg-save"')}`))}
    ${section('log', 'Diagnostic log',
      row('log', 'Keep a log', info('log', 'What Harness Manager did and what went wrong (a usage file it could not read, a failed backup), with the time. It holds no keys or logins.<br><b>Default: on, Warnings.</b> Information also records backups, restores and the like.'), 'Record diagnostic events; nothing leaves this computer.', sw('log_enabled', s.log_enabled, 'Keep a log', 'st-log'))
      + row('loglevel', 'Level', info('loglevel', '<b>Errors:</b> only failures. <b>Warnings (the default):</b> failures and things worth a look. <b>Everything:</b> also backups, restores and the like.'), 'How much it records.', chips('log_level', s.log_level, [['error', 'Errors'], ['warn', 'Warnings'], ['info', 'Everything']], 'st-loglevel'))
      + row('logfile', 'Log file', info('logfile', 'HarnessManager.log in the data folder.<br><b>Default: the data folder.</b>'), 'HarnessManager.log in the data folder; it holds no keys or logins.', btn('Open log', 'log-open', 'data-testid="log-open"')))}
    ${section('maintenance', 'Maintenance',
      row('find', 'Find tools again', info('find', 'Adds a main profile for each tool found on this computer that has none yet.<br><b>Default: run manually.</b>'), 'Add the main profiles of detected tools.', btn('Find tools', 'detect'))
      + row('import', 'Old launcher', info('import', 'Reads claude-accounts.json of the old PowerShell launcher on this computer. Keys can only be read on the computer that saved them.<br><b>Default: run manually.</b>'), 'Import accounts and keys from <span class="mono">claude-accounts.json</span> on this computer.', btn('Import…', 'import-ps', 'data-testid="import-ps"'))
      + row('tours', 'Tours', info('tours', 'Each page has a tour of its parts, started from the ? at the top. Skip all tours turns them off; this turns them back on.<br><b>Default: on.</b>'), 'The ? at the top replays this page\'s tour.', btn('Show the tours again', 'guides-reset', 'data-testid="guides-reset"'))
      + row('relink', 'Link copies to the main profile', info('relink', 'Accounts made as independent copies of the tool\'s settings are replaced with links to the main profile, so one change reaches every account. The originals are kept in .hrm-backup beside them.<br><b>Default: run manually.</b>'), 'Replace old independent copies with links; keep originals in .hrm-backup.', btn('Link copies…', 'relink')))}
    ${section('computers', 'Computers', `<div data-testid="computers">${c ? computersSection(c, s, st, ts) : '<div class="b-setting"><span class="muted">Computers could not be read.</span></div>'}</div>`)}
    ${section('backups', 'Backups and archive', b ? backupsSection(b, ar, s) : '<div class="b-setting"><span class="muted">Backups could not be read.</span></div>')}
  </div>`);
  // #settings?tab=computers (the chip in the top bar, Apps › Settings › Updates): that section into view
  if (params && params.tab) jump(params.tab);
});
const jump = (id) => { const d = document.getElementById('sec-' + id); if (!d) return; d.open = true; shut.delete(id); d.scrollIntoView({ block: 'start' }); };
action('set-jump', (el) => jump(el.dataset.v));

// ---------- Computers: this computer's name, how it syncs, what travels, and a card per computer ----------
function computersSection(c, s, st, ts) {
  const on = syncOn(c), now = Date.now(), n = computers(c).length;
  const hub = c.hub || {}, hubOn = on && c.mode === 'hub';
  const card = (d, i) => {
    const me = d === c.this, seq = d.last_seq || 0, done = me ? (d.published_seq ?? seq) : (d.applied_seq || 0), pct = seq ? Math.min(100, Math.round((done / seq) * 100)) : 100;
    const stale = !me && !heardOn(d, now);
    const heard = me ? 'writing now' : d.connected ? 'connected' : d.last_seen ? (now - d.last_seen < 90000 ? 'writing now' : 'heard ' + ago(d.last_seen)) : 'not heard from yet';
    const facts = [d.projects != null ? `${d.projects} ${d.projects === 1 ? 'project' : 'projects'}` : '', d.accounts != null ? `${d.accounts} ${d.accounts === 1 ? 'account' : 'accounts'}` : '', `${(d.running || []).length} running`].filter(Boolean);
    // a peer: how it is reached now, when it was paired and last pulled (the ⓘ), its address, and Forget
    const peer = me ? '' : `<span class="muted" data-testid="transport">${esc(transportLine(c, d))} ${info('peer-' + d.id, `${d.direct ? `<b>Paired</b> ${d.paired_at ? esc(stamp(d.paired_at)) : 'yes'}` : '<b>Not paired</b>: known through the folder only'}<br><b>Last pull</b> ${d.last_pull ? esc(ago(d.last_pull)) : 'never'}${d.address ? `<br><b>Address</b> ${esc(d.address)}` : ''}${d.relayed_by ? `<br><b>Relayed through</b> ${esc(nameOf(c, d.relayed_by))}` : ''}`)}</span>
      <div class="page-actions">${d.direct ? `${field('addr-' + esc(d.id), 'Address', `class="mono" value="${esc(d.address || '')}" placeholder="name:port" maxlength="120" title="Where to reach ${esc(d.name)} when not on this network: a VPN name, with the port" data-testid="addr-${esc(d.id)}"`)}${btn('Save', 'peer-addr', `data-id="${esc(d.id)}" data-testid="addr-save-${esc(d.id)}"`)}` : ''}${btn('Forget…', 'peer-forget', `data-id="${esc(d.id)}" data-name="${esc(d.name)}" data-testid="forget-${esc(d.id)}"`)}</div>`;
    return `<article class="b-computer" data-key="computer-${esc(d.id || 'this')}" data-testid="computer-${esc(d.id || 'this')}"><div class="b-name"><i class="pc-dot c${colorIdx(c, d.id)}"></i><b>${esc(d.name)}</b>${me ? '<span class="plan-pill">This one</span>' : ''}</div>
      <span class="${stale ? 'b-warn' : 'muted'}">${esc([me ? { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[st.os] || st.os : d.os, me ? st.version : d.app_version, heard].filter(Boolean).join(' · '))}</span>
      <div role="progressbar" aria-label="${esc(d.name)} entries ${me ? 'published' : 'applied'}" aria-valuenow="${done}" aria-valuemax="${Math.max(1, seq)}" aria-valuemin="0">${winTrack(pct, pct < 100 && stale ? 'warn' : '')}</div>
      <span>${done.toLocaleString()} of ${seq.toLocaleString()} entries ${me ? 'published' : 'applied here'}</span><span class="muted">${esc(facts.join(' · '))}</span>${peer}</article>`;
  };
  return row('devname', 'This computer', info('devname', 'The name the other computers show for this one, in their lists, on sessions and on the chip at the top.<br><b>Default: the computer name</b> Windows or macOS gives it.'), 'The name your other computers see.',
      `${field('st-devname', 'Name', `value="${esc(c.this.name)}" maxlength="40" data-testid="st-devname"`)}${btn('Rename', 'computers-set', 'data-key="device_name" data-from="st-devname" data-testid="st-devname-save"')}`)
    + row('syncmode', 'How your computers sync', info('syncmode', 'Pick one per computer. Each computer writes its own journal there and reads the others\'; keys and logins never go in it. Switching later keeps everything, because the journal is the same either way.<br><b>Default: off.</b>'), 'Pick one transport; changing it keeps merged history.', syncChoices(c))
    + (hubOn || hub.ring ? row('ring', 'Recovery key', info('ring', 'The ring key seals every chunk, presence report and transcript before it goes to the hub, so the hub stores what it cannot read. It is made on the first computer that joins the hub, kept in Credential Manager, carried to paired computers with the pairing, and shown once as the recovery key: 8 groups of 4 letters and digits for your password manager. A computer that is not paired needs it typed in.<br><b>Default: made on the first computer.</b>'), hub.ring ? 'Stored on this computer; paired computers receive it with the pairing. Keep it in your password manager for a computer that is not paired, or a fresh install.' : 'No ring key: this computer cannot read or write the hub. Enter the recovery key shown on the computer that first joined, or pair with it.',
      `${hub.ring ? btn('Show again…', 'ring-show', 'data-testid="ring-show"') : ''}<button class="${hub.ring ? 'btn-secondary' : 'btn-primary'}" data-act="ring-enter" data-testid="ring-enter">Enter recovery key…</button>`) : '')
    + row('transcripts', 'Carry transcripts', info('transcripts', `Also share this computer's session transcripts, so they can be read and resumed on another computer. A transcript over 20 MB is not carried; the folder holds up to 200 MB of this computer's transcripts, the oldest going first.${ts ? `<br>Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB${ts.skipped && ts.skipped.length ? ` · ${ts.skipped.length} skipped` : ''}` : ''}<br><b>Default: on.</b>`), `${ts ? `Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB · ${(ts.skipped || []).length} skipped. ` : ''}Over 20 MB is skipped; the store holds 200 MB.`, sw('sync_transcripts', s.sync_transcripts !== false, 'Carry transcripts', 'st-transcripts', 'computers-set'))
    + row('presence', 'Presence', info('presence', 'Writes what is running here every minute while the app is open, so the other computers show it in Running now.<br><b>Default: on.</b>'), 'Share what runs here every minute.', sw('sync_presence', s.sync_presence !== false, 'Share what runs here', 'st-presence', 'computers-set'))
    + row('direct', 'Direct connection', info('direct', 'Finds your other computers on this network (mDNS) or at an address you give (a VPN name), after pairing once with a six-digit code, and talks to them over TLS: what runs there every ten seconds, their history the moment it is written. The folder keeps working underneath; a computer reached neither way is still merged through it.<br><b>Default: off.</b>'), 'Paired computers talk over TLS. Presence updates every 10 seconds; the folder keeps working.',
      `${sw('direct_connection', s.direct_connection, 'Connect directly', 'st-direct', 'computers-set')}${btn('Pair a computer…', 'pair', 'data-testid="pair"')}<button class="btn-secondary" data-act="listen-port" data-testid="listen-port-change">Listen port: <span data-testid="listen-port">${c.port || 'chosen at first use'}</span>…</button>`)
    + `<div class="b-toolbar"><b>${n} ${n === 1 ? 'computer' : 'computers'}</b><span class="muted">${on ? `Synced through a ${c.mode === 'hub' ? 'hub' : 'folder'}${c.last_run ? ' · last round ' + ago(c.last_run) : ''}` : 'Sync is off: only this computer'}</span>${on ? btn('Sync now', 'computers-now', 'data-testid="computers-now"') : ''}${btn('First-run choices…', 'firstrun-open')}</div>
      ${c.error ? `<div role="alert" class="notice-bar warn" data-testid="sync-error">${esc(c.error)}</div>` : ''}
      <div class="b-computers" data-testid="computer-cards">${computers(c).map(card).join('')}</div>`;
}
// a switch or the name: saved through computers_set, which answers with the new status
action('computers-set', async (el) => {
  const value = el.type === 'checkbox' ? el.checked : document.getElementById(el.dataset.from).value.trim();
  if (el.type !== 'checkbox' && !value) return toast('Enter a computer name.', true);
  if (await mutate(() => call('computers_set', { [el.dataset.key]: value }), 'Saved') === undefined && el.type === 'checkbox') el.checked = !el.checked;
});
action('sync-folder-pick', async () => {
  const p = await pickFolder();
  if (p && await mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p) !== undefined) (await import('./restore.js')).welcomeBack(p);
});
action('sync-off', async () => { if (await confirmDialog('Stop syncing', 'Leave the folder? Merged history stays on this computer, and the folder keeps what was written.', 'Leave')) mutate(() => call('computers_set', { sync_mode: 'off' }), 'Sync is off; the folder keeps what was written'); });
action('computers-now', async () => {
  const r = await attempt(() => call('computers_now'));
  if (!r) return;
  const n = (r.peers || []).length;
  toast(r.error ? 'Sync round: ' + r.error : `Synced with ${n} other ${n === 1 ? 'computer' : 'computers'}`, !!r.error);
  changed();
});
// the port the direct connection listens on, a peer's address, and forgetting a peer
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
action('peer-addr', (el) => mutate(() => call('computers_set_peer', { id: el.dataset.id, address: document.getElementById('addr-' + el.dataset.id).value.trim() }), 'Saved'));
action('peer-forget', async (el) => {
  if (await confirmDialog('Forget ' + el.dataset.name, `Forget <b>${esc(el.dataset.name)}</b>? This computer stops connecting to it and drops its certificate; what was merged from it stays. Pair again to reconnect.`, 'Forget')) mutate(() => call('computers_forget', { id: el.dataset.id }), 'Forgotten');
});
// presence arrives every 10 s while a peer is connected: the page follows in place (the reconciler keeps what is being edited)
document.addEventListener('presence', () => { if (S.route.view === 'settings' && S.state && !dialogOpen()) render(); });

// ---------- Backups and archive ----------
function backupsSection(b, ar, s) {
  const folderOn = !!(S.cache.computers && S.cache.computers.mode === 'folder');
  const days = s.archive_days ?? 30, loc = (ar && ar.locations) || [];
  return row('data', 'Data folder', info('data', 'The account list (unless it lives elsewhere), settings, the usage database, backups and the log.<br><b>Default: this computer\'s app data.</b>'), 'The local usage index and shared setup.', `<span class="mono b-command" title="${esc(b.data)}">${esc(b.data)}</span>${btn('Open', 'open-folder', `data-path="${esc(b.data)}"`)}`)
    + row('bk', 'Automatic backups', info('bk', 'A backup holds the account list, settings, what Harness Manager wrote into each tool, the folders of the skills it manages, and the history: every request, session, limit reading and tag, with the computer each came from, and the archived sessions. API keys and logins stay in Windows Credential Manager and are never in a backup.<br><b>Default: every 24 hours, keeping 10.</b>'), 'Registry, usage and limits; keys and logins never enter backups.',
      `${chips('backup_hours', s.backup_hours, [[0, 'Off'], [6, '6 h'], [12, '12 h'], [24, '24 h'], [168, 'Week']], 'st-bkhours')}<label class="b-switch">Keep <select data-act="set-now" data-key="backup_keep" aria-label="Backups kept" data-testid="st-bkkeep">${[3, 5, 10, 20, 50].map((v) => `<option value="${v}" ${s.backup_keep === v ? 'selected' : ''}>${v}</option>`).join('')}</select></label>`)
    + row('bkfolder', 'Also in the sync folder', info('bkfolder', 'Every backup is also copied to <span class="mono">backups\\&lt;this computer&gt;</span> in the sync folder, keeping as many there as here, so a fresh computer finds its history beside the journal.<br><b>Default: on.</b>'), folderOn ? `Copy backups to ${esc(s.sync_folder)}.` : 'Once a sync folder is chosen.', sw('backup_in_sync_folder', s.backup_in_sync_folder !== false, 'Copy backups there', 'st-bk-folder'))
    + row('bktr', 'Include transcripts', info('bktr', 'The zip also holds this computer\'s session transcripts, compressed, so a restore brings the conversations back too. Backups get larger.<br><b>Default: off.</b>'), 'Include session transcripts in backups.', sw('backup_transcripts', !!s.backup_transcripts, 'Include transcripts', 'st-bk-transcripts'))
    + `<div class="b-toolbar"><b>Backups · ${b.list.length} · ${kb(b.list.reduce((a, x) => a + x.size, 0))}</b>${btn('Import…', 'backup-import', 'data-testid="backup-import"')}${btn('Export…', 'backup-export', 'data-testid="backup-export"')}${btn('Back up now', 'backup-now', 'data-testid="backup-now"')}</div>
      ${b.list.map((x) => `<div class="account-session" data-key="backup-${esc(x.name)}" data-testid="backup-${esc(x.label)}"><div><b>${esc(x.label)}</b><div class="muted">${esc(stamp(x.at))} · ${kb(x.size)}</div></div><div class="page-actions">${btn('Rename', 'backup-rename', `data-name="${esc(x.name)}" data-label="${esc(x.label)}"`)}${btn('Restore…', 'backup-restore', `data-name="${esc(x.name)}" data-testid="restore-${esc(x.label)}"`)}${btn('Delete…', 'backup-delete', `data-name="${esc(x.name)}"`)}</div></div>`).join('') || '<div class="b-empty"><b>No backups yet.</b></div>'}
      <div class="b-toolbar"><b>In the sync folder · ${(b.in_sync_folder || []).length} · every computer's</b></div>
      ${(b.in_sync_folder || []).map((x) => `<div class="account-session" data-key="folder-backup-${esc(x.path)}" data-testid="folder-backup-${esc(x.label)}"><div><b>${esc(x.label)} · ${esc(x.device_name)}</b><div class="muted">${esc(stamp(x.at))} · ${kb(x.size)}</div></div>${btn('Restore…', 'restore-src', `data-zip="${esc(x.path)}"`)}</div>`).join('') || '<span class="muted">No shared backups.</span>'}`
    + row('arch', 'Archive sessions older than', info('arch', 'Transcripts of sessions older than this move from the tools\' folders into the archive, compressed by project and month, once a day while the app runs and with Archive now. Each copy is checked before the original is deleted. The session stays in the list with its numbers and stays readable; it cannot be resumed from the archive. Usage is never touched.<br><b>Default: 30 days.</b> Never keeps every transcript where its tool put it.'), 'Archive old transcripts; keep the usage index.',
      `${chips('archive_days', days, [[0, 'Never'], [30, '30 days'], [90, '90 days'], [365, '1 year']], 'st-archive-days')}${field('st-archive-custom', 'Custom days', `class="mono" type="number" min="0" max="36500" value="${days}" data-testid="st-archive-custom"`)}${btn('Save', 'archive-days-custom', 'data-testid="st-archive-custom-save"')}`)
    + row('archloc', 'Archive location', info('archloc', 'Where the copies go: the sync folder (<span class="mono">archive\\&lt;this computer&gt;</span>, so every computer and a fresh install can read them), a folder of your choice, or both.<br><b>Default: the sync folder.</b> With neither, nothing is archived.'), 'Copy archived transcripts to these locations.',
      `${sw('archive_in_sync_folder', s.archive_in_sync_folder !== false, `Sync folder${folderOn ? '' : ' (none chosen yet)'}`, 'st-archive-sync')}<span class="mono" title="${esc(s.archive_dir)}" data-testid="st-archive-dir">${esc(s.archive_dir || 'No folder chosen')}</span>${btn(s.archive_dir ? 'Change…' : 'Choose…', 'archive-dir-pick', 'data-testid="st-archive-dir-pick"')}${s.archive_dir ? btn('None', 'archive-dir-clear', 'data-testid="st-archive-dir-clear"') : ''}`)
    + `<div class="b-toolbar"><span>${ar ? `${ar.archived} archived · ${kb(ar.bytes)}${ar.last && ar.last.at ? ` · last run ${esc(when(ar.last.at))}` : ''}` : 'Nothing archived yet.'}</span>${btn('Archive now', 'archive-now', 'data-testid="archive-now"')}${btn('Restore…', 'restore-pick', 'data-testid="restore-open"')}</div>
      <span class="muted">${loc.map((l) => `${l.kind === 'sync_folder' ? 'sync folder' : 'folder'}: ${l.sessions} (${l.on ? 'on' : 'off'})`).join(' · ') || 'Restore merges a backup or the sync folder\'s history into this computer; nothing is counted twice.'}</span>`;
}
action('archive-now', async () => {
  const r = await attempt(() => call('archive_now'));
  if (!r) return;
  toast(r.why ? `Nothing archived: ${r.why}` : `${r.archived} ${r.archived === 1 ? 'session' : 'sessions'} moved to the archive`);
  changed();
});
action('archive-days-custom', () => { const v = +document.getElementById('st-archive-custom').value; if (v >= 0) mutate(() => call('settings_set', { key: 'archive_days', value: Math.floor(v) }), 'Saved'); });
action('archive-dir-pick', async () => { const p = await pickFolder(); if (p) mutate(() => call('settings_set', { key: 'archive_dir', value: p }), 'Saved'); });
action('archive-dir-clear', () => mutate(() => call('settings_set', { key: 'archive_dir', value: '' }), 'Saved'));
const restored = (r) => {
  const sk = r.restored.skills.length;
  toast(`Restored${sk ? `, with ${sk} skill ${sk === 1 ? 'folder' : 'folders'} put back` : ''}. The state before is kept as a backup named Before restore.`);
  changed();
};
action('backup-now', () => mutate(() => call('backup_create', { label: 'Manual' }), 'Backed up'));
action('backup-delete', async (el) => { if (await confirmDialog('Delete backup', 'Delete this backup? It cannot be undone.', 'Delete')) mutate(() => call('backup_delete', { name: el.dataset.name })); });
action('backup-rename', (el) => {
  openDialog(`${dlgHead('Rename backup')}<div class="form-field"><label for="bk-l">Label</label><input id="bk-l" value="${esc(el.dataset.label)}" maxlength="60"></div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="bk-ok">Save</button></span></div>`, { size: 'sm', label: 'Rename backup' });
  document.getElementById('bk-ok').onclick = async () => { if (await attempt(() => call('backup_rename', { name: el.dataset.name, label: document.getElementById('bk-l').value }))) { closeDialog(); changed(); } };
});
action('backup-restore', async (el) => {
  if (!(await confirmDialog('Restore backup', 'The account list, settings and what Harness Manager wrote into each tool go back to this backup, and skill folders missing from the store come back. The state now is kept as a backup named Before restore first. Keys and logins are not touched.', 'Restore', false))) return;
  const r = await attempt(() => call('backup_restore', { name: el.dataset.name }));
  if (r) restored(r);
});
action('backup-export', async () => {
  const to = await saveFile(`Harness Manager backup ${new Date().toISOString().slice(0, 10)}.zip`, { name: 'Backup', extensions: ['zip'] });
  if (to) attempt(() => call('backup_export', { path: to }), 'Exported: keep it somewhere safe; it holds no keys or logins');
});
action('backup-import', async () => {
  const from = await pickFile({ name: 'Backup', extensions: ['zip'] });
  if (!from) return;
  if (!(await confirmDialog('Import backup', `Everything in <span class="mono">${esc(from)}</span> is put back, as for a backup made here. The state now is kept as a backup first; keys and logins stay here.`, 'Restore', false))) return;
  const r = await attempt(() => call('backup_import', { path: from }));
  if (r) restored(r);
});
action('log-open', () => attempt(() => call('log_open'))); // open-folder is accounts.js's action, shared
// Harness Manager's own update: found at launch (the self-update event) or by Check for updates
let selfUpd = null;
export function setSelfUpdate(v) { selfUpd = v; const dot = document.getElementById('settings-upd'); if (dot) dot.hidden = !(v && v.version); }
action('self-check', async () => {
  const v = await attempt(() => call('self_update_check'));
  if (!v) return;
  setSelfUpdate(v);
  if (!v.version) toast(`Harness Manager ${v.current} is the newest version`);
  render();
});
action('self-install', async (el) => {
  if (!(await confirmDialog('Update and restart', `${selfUpd && selfUpd.notes ? esc(selfUpd.notes) + ' ' : ''}Install the signed update ${esc((selfUpd || {}).version || '')} and restart?`, 'Update and restart', false))) return;
  el.disabled = true;
  el.textContent = 'Updating…';
  // on success the app restarts, so only a failure comes back, or "nothing newer" when the offer was out of date
  const v = await attempt(() => call('self_update_install'));
  if (!v) { el.disabled = false; el.textContent = 'Update and restart…'; return; }
  if (!v.version) { setSelfUpdate(v); toast(`Harness Manager ${v.current} is the newest version`); render(); }
});
action('set-now', async (el) => {
  // a switch's state, a chip's value, or a dropdown's choice (numbers as numbers)
  const raw = el.tagName === 'SELECT' ? el.value : el.dataset.v;
  const value = raw === undefined ? el.checked : /^\d+$/.test(raw) ? +raw : raw;
  if (await mutate(() => call('settings_set', { key: el.dataset.key, value }), 'Saved') === undefined && el.type === 'checkbox') el.checked = !el.checked;
});
action('live-now', async (el) => { if (!(await mutate(() => call('live_quota_set', { on: el.checked }), 'Saved'))) el.checked = !el.checked; });
action('autostart', async (el) => {
  const on = await mutate(() => call('autostart_set', { on: el.checked }));
  if (on === undefined) { el.checked = !el.checked; return; }
  toast(on ? 'Harness Manager starts in the tray when you sign in' : 'It no longer starts at sign-in');
});
action('guides-reset', () => { resetGuides(); toast('Each page will show its tour again'); render(); });
// a typed setting (the port, the account list's place): saved by the button beside it
action('set-field', async (el) => {
  const raw = document.getElementById(el.dataset.from).value.trim(), value = el.dataset.key === 'router_port' ? +raw : raw;
  if (el.dataset.key === 'router_port' && !(value >= 1 && value <= 65535)) return toast('A port is 1 to 65535', true);
  mutate(() => call('settings_set', { key: el.dataset.key, value }), 'Saved');
});
action('import-ps', async () => {
  if (!(await confirmDialog('Import old launcher', 'Import the accounts and keys from <span class="mono">claude-accounts.json</span> on this computer? Keys can be read only on the computer that saved them.', 'Import', false))) return;
  const r = await mutate(() => call('import_ps', {}));
  if (r) toast(`Imported ${r.accounts.length} accounts and ${r.keys.length} keys${r.skipped.length ? '; skipped ' + r.skipped.length : ''}`);
});
action('detect', async () => { const r = await mutate(() => call('detect', {})); if (r) toast(r.main_profiles.length ? `Added ${r.main_profiles.join(', ')}` : 'No new tools found'); });
action('relink', async () => {
  if (!(await confirmDialog('Link copies to the main profile', 'Existing copies move to .hrm-backup; the main profile is linked in their place.', 'Replace', false))) return;
  const r = await mutate(() => call('sync', { relink: true }));
  if (r) toast(r.relinked.length ? `Linked: ${r.relinked.join('; ')}` : 'Nothing to link; accounts are in step');
});

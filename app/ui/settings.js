// Settings, as the 1.10 drawing draws it (docs/project-review-2026-11/mockups/a-settings.html): the head, then two
// columns: a list of the sections with "Search settings" on top, and the chosen section alone as a card ("N settings")
// of rows: a label with its ⓘ (what the help line leaves out, and the default), a one-line help, the control at the
// right. The chosen section is in the address (#settings?tab=backups), so Back and a link to a section work. Typing
// in the search shows the rows of every section whose label or help has the words. Choices save at once; typed fields
// have a button beside them. Computers is a page of its own (computers-page.js, which draws its rows with the parts
// exported here). Harness Manager's own update lives here.

import { S, call, view, go, toast, attempt, mutate, changed, openDialog, closeDialog, dlgHead, confirmDialog, action, render, pickFolder, saveFile, pickFile, loadState, patch, keyStore } from './core.js';
import { esc, when } from './lib.js';
import { computersStatus } from './computers.js';
import { pageHead, seg, plural } from './parts.js';

// ---------- the parts ----------
/** An ⓘ that explains a setting on hover or keyboard focus. */
export const info = (id, html) => `<span class="b-tip"><button type="button" class="icon-btn" aria-label="About this setting" aria-describedby="tip-${esc(id)}" data-testid="tip-${esc(id)}">i</button><span role="tooltip" id="tip-${esc(id)}">${html}</span></span>`;
const line = (key, title, tip, text, ctl) => `<div class="srow" data-key="${key}"><div class="what"><div class="row"><b>${title}</b>${tip || ''}</div><p>${text}</p></div><div class="ctl">${ctl}</div></div>`;
/** A row: the label with its ⓘ, the one-line help, the control at the right. */
export const row = (key, title, tip, text, ctl) => line('setting-' + key, title, tip, text, ctl);
/** A row that is not a setting (the list of backups, the archive): drawn and searched like one, not counted as one. */
const block = (key, title, text, ctl) => line('block-' + key, title, '', text, ctl);
/** A box with its words, saved as soon as it is ticked (Settings), and a switch that says On or Off (Computers). */
const sw = (key, on, label, testid, act = 'set-now') => `<label class="chk"><input type="checkbox" data-act="${act}" data-key="${key}" ${on ? 'checked' : ''} data-testid="${testid}"> ${label}</label>`;
export const toggle = (key, on, label, testid, act = 'set-now') => `<label class="sw"><input type="checkbox" role="switch" aria-label="${esc(label)}" data-act="${act}" data-key="${key}" ${on ? 'checked' : ''} data-testid="${testid}"><i></i><b></b></label>`;
/** A choice that saves as soon as it is made. */
const pick = (key, cur, opts, testid) => seg(key.replaceAll('_', ' '), opts, cur, (v) => `data-act="set-now" data-key="${key}" data-v="${v}"`, testid);
const field = (id, label, extra = '') => `<input id="${id}" aria-label="${esc(label)}" ${extra}>`;
const btn = (label, act, extra = '') => `<button class="btn sm" data-act="${act}" ${extra}>${label}</button>`;
const kb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });

// ---------- the sections: [id, name, rows] ----------
function sections(st, b, ar, path) {
  const s = st.settings;
  // the sign-in entry and the choice of terminal are Windows things; macOS opens Terminal
  const win = !st.os || st.os === 'windows';
  return [
    ['appearance', 'Appearance',
      row('brand', 'Theme', info('brand', '<b>Harness (the default):</b> porcelain and night, Inter.<br><b>VektraGrid:</b> ivory and deep green, Garamond headings; deep teal leads, plum marks the router, sand gold is the warning.<br>Light and dark follow the choice below.'), 'The look of this window.', pick('brand', s.brand || 'hm', [['hm', 'Harness'], ['vg', 'VektraGrid']], 'st-brand'))
      + row('theme', 'Light and dark', info('theme', '<b>System (the default):</b> follows the light or dark setting of Windows or macOS.<br><b>Light</b> or <b>Dark</b>: always that.'), 'Follow the system, or keep one.', pick('theme', s.theme, [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']], 'st-theme'))
      + row('quota', 'Limits shown as', info('quota', '<b>Left (the default):</b> how much of each limit you still have, as the tools show it.<br><b>Used:</b> how much has gone.'), 'Applies to every ring and bar.', pick('quota_display', s.quota_display, [['left', 'Left'], ['used', 'Used']], 'st-quota'))
      + row('motion', 'Moving routes', info('motion', 'Pulses along the live routes on the Overview, as fast as their traffic. Off, or with reduced motion in Windows or macOS, the routes are a still picture.<br><b>Default: on.</b>'), "Off, or the system's reduced motion, keeps them still.", sw('moving_routes', s.moving_routes, 'Animate the routes', 'st-motion'))
      + row('textsize', 'Text size', info('textsize', '<b>Compact:</b> everything a little smaller, for more on one screen.<br><b>Comfortable (the default):</b> body text at 13 px, nothing under 12 px.<br><b>Large:</b> everything a little larger.'), 'Applies to every page.', pick('text_size', s.text_size || 'comfortable', [['compact', 'Compact'], ['comfortable', 'Comfortable'], ['large', 'Large']], 'st-textsize'))],
    ['window', win ? 'Window and terminal' : 'Window',
      row('tray', 'Close to the tray', info('tray', '<b>On (the default):</b> the window\'s X hides Harness Manager to the tray; usage and limits keep refreshing, and the tray icon opens it again. Quit from the tray menu.<br><b>Off:</b> the X quits.'), 'The window hides and the app keeps refreshing.', sw('close_to_tray', s.close_to_tray, 'Keep running in the tray', 'st-tray'))
      + (win ? row('auto', 'Launch at sign-in', info('auto', 'Starts Harness Manager in the tray when you sign in to Windows, so usage and limits are current when you open it.<br><b>Default: off.</b>'), 'Starts hidden in the tray.', `<label class="chk"><input type="checkbox" data-act="autostart" ${st.autostart ? 'checked' : ''} data-testid="st-autostart"> Start with Windows</label>`) : '')
      + (win ? row('term', 'Terminal', info('term', '<b>Windows Terminal</b> (the default): Start and Resume open a new Windows Terminal window running the account.<br><b>Console window</b>: the classic console, for computers without Windows Terminal or if you prefer it.'), 'Where Start and Resume open a console.', pick('terminal', s.terminal, [['wt', 'Windows Terminal'], ['console', 'Console']], 'st-term')) : '')
      + row('cli-path', 'hrm on the PATH', '', !path ? 'Could not read the terminal PATH.' : path.on_path ? 'New terminals find this copy of hrm.' : 'New terminals do not find this copy of hrm.',
        (path?.can_add ? btn('Add', 'cli-path', 'data-add="true" aria-label="Add hrm to the PATH"') : '')
        + (path?.can_remove ? btn('Remove', 'cli-path', 'aria-label="Remove hrm from the PATH"') : ''))],
    ['updates', 'Updates',
      row('self', 'Harness Manager', info('self', 'New versions are looked for at launch and every hour; only signed updates are installed.<br><b>Default: checks at launch and every hour.</b>'), selfUpd && selfUpd.version ? `Version <b>${esc(selfUpd.version)}</b> is available; you have ${esc(st.version)}.${selfUpd.notes ? ' ' + esc(selfUpd.notes) : ''} Signed updates only.` : `Version ${esc(st.version)}${selfUpd ? ' · the newest' : ''}. Signed updates only.`,
        selfUpd && selfUpd.version ? '<button class="btn sm primary" data-act="self-install" data-testid="self-install">Update and restart…</button>' : btn('Check for updates', 'self-check', 'data-testid="self-check"'))
      + row('upd', 'New versions of your tools', info('upd', 'Asks each vendor for the newest Claude Code, Codex, OpenCode and Antigravity CLI when Harness Manager starts; Apps in the sidebar shows how many have one.<br><b>Default: off.</b> Apps &gt; Check for updates looks at any time.'), 'Ask vendors for new versions when the app starts.', sw('check_updates_at_launch', s.check_updates_at_launch, 'Check at launch', 'st-updcheck'))],
    ['limits', 'Limits',
      row('refresh', 'Refresh', info('refresh', 'How often, in minutes, Harness Manager reads new usage from the tools\' logs and the cached limits, in the background.<br><b>Default: 5.</b> A smaller number updates sooner but reads the logs more often. The ↻ at the top reads them at once whatever this is.'), 'Read usage and cached limits in the background.', pick('refresh_minutes', String(s.refresh_minutes), [['1', '1 min'], ['5', '5 min'], ['15', '15 min']], 'st-refresh'))
      + row('live', 'Live Claude limits', info('live', '<b>Off (the default):</b> limits are what Claude Code last saved, so they are as fresh as the account\'s last run; a window whose reset time has passed shows as empty.<br><b>On:</b> when those numbers are more than 10 minutes old, Harness Manager asks Anthropic for current ones with the login Claude Code saved, while it is valid. It never refreshes or copies that login, never starts a session and never reads conversations.'), 'Read current limits with the login Claude Code saved.', `<label class="chk"><input type="checkbox" id="st-live" data-act="live-now" ${st.live_quota ? 'checked' : ''} data-testid="st-live"> Ask Anthropic for current limits</label>`)],
    ['connections', 'Connections',
      row('port', 'Router port', info('port', 'The port of the local router that routed accounts talk to. It listens on 127.0.0.1 only, so nothing outside this computer can reach it.<br><b>Default: 15731.</b> Change it only if another program uses that port; routed accounts use the new port the next time they start.'), '127.0.0.1 only.', `${field('st-port', 'Port', `class="mono" type="number" min="1" max="65535" value="${s.router_port}"`)}${btn('Change', 'set-field', 'data-key="router_port" data-from="st-port" data-testid="st-port-save"')}`)
      + row('reg', 'Account list', info('reg', 'Where profiles.json lives: your accounts, providers, routes, MCP servers, skills and prompts. No keys or logins are ever stored in it.<br><b>Empty (the default):</b> it stays in this computer\'s Harness Manager folder (%APPDATA%\\HarnessManager). Enter a path in a OneDrive folder to share the same accounts with your other computer; each computer keeps its own keys and logins.'), 'Share profiles through a folder; keys and logins stay here.', `${field('st-reg', 'profiles.json path', `class="mono wide" value="${esc(s.registry_path)}" placeholder="${esc(st.registry_path)}"`)}${btn('Move…', 'set-field', 'data-key="registry_path" data-from="st-reg" data-testid="st-reg-save"')}`)],
    ['log', 'Log',
      row('log', 'Keep a log', info('log', 'What Harness Manager did and what went wrong (a usage file it could not read, a failed backup), with the time. It holds no keys or logins.<br><b>Default: on, Warnings.</b> Information also records backups, restores and the like.'), 'Record diagnostic events; nothing leaves this computer.', sw('log_enabled', s.log_enabled, 'Keep a log', 'st-log'))
      + row('loglevel', 'Level', info('loglevel', '<b>Errors:</b> only failures. <b>Warnings (the default):</b> failures and things worth a look. <b>Everything:</b> also backups, restores and the like.'), 'How much it records.', pick('log_level', s.log_level, [['error', 'Errors'], ['warn', 'Warnings'], ['info', 'Everything']], 'st-loglevel'))
      // (no ⓘ: its popover said what this line says)
      + row('logfile', 'Log file', '', 'HarnessManager.log in the data folder; no keys or logins.', btn('Open log', 'log-open', 'data-testid="log-open"'))],
    ['maintenance', 'Maintenance',
      row('find', 'Find tools again', info('find', 'Adds a main profile for each tool found on this computer that has none yet.<br><b>Default: run manually.</b>'), 'Add the main profiles of detected tools.', btn('Find tools', 'detect'))
      + row('import', 'Old launcher', info('import', 'Reads claude-accounts.json of the old PowerShell launcher on this computer. Keys can only be read on the computer that saved them.<br><b>Default: run manually.</b>'), 'Import accounts and keys from <span class="mono">claude-accounts.json</span>.', btn('Import…', 'import-ps', 'data-testid="import-ps"'))
      + row('relink', 'Link copies to the main profile', info('relink', 'Accounts made as independent copies of the tool\'s settings are replaced with links to the main profile, so one change reaches every account. The originals are kept in .hrm-backup beside them.<br><b>Default: run manually.</b>'), 'Replace independent copies with links to the main profile.', btn('Link copies…', 'relink'))],
    ['backups', 'Backups', b ? backupsSection(b, ar, s) : '<p class="set-none">Backups could not be read.</p>'],
  ];
}

// the sections as last read, the one chosen, and what is typed into Search settings (kept while the page redraws)
let drawn = [], chosen = 'appearance', query = '';
const count = (body) => (body.match(/data-key="setting-/g) || []).length;
const pane = (id, title, body, note) => `<section class="card panel-card set-pane" id="sec-${id}" data-testid="sec-${id}"><div class="pane-head"><h2 class="section">${title}</h2><span class="small muted">${note}</span></div>${body}</section>`;
/** The right side: the chosen section, or with words in the search the rows of every section that have them. */
function panes() {
  const q = query.trim().toLowerCase();
  if (!q) { const [id, title, body] = drawn.find(([x]) => x === chosen); return pane(id, title, body, plural(count(body), 'setting')); }
  const t = document.createElement('template');
  // a row is found by its label and its help line
  const has = (r) => (r.querySelector('.what b').textContent + ' ' + r.querySelector('.what p').textContent).toLowerCase().includes(q);
  return drawn.map(([id, title, body]) => {
    t.innerHTML = body;
    const rows = [...t.content.querySelectorAll('.srow')].filter(has);
    return rows.length ? pane(id, title, rows.map((r) => r.outerHTML).join(''), `${rows.length} found`) : '';
  }).join('') || '<section class="card panel-card set-pane" data-testid="set-none"><p class="set-none">No setting matches.</p></section>';
}
/** What Search settings looks through, for the command palette (palette.js): every section with its rows, each row
 * by the label and the help line the search above matches: [{ id, title, rows: [{ label, help }] }]. From the sections
 * last drawn; when the page has not been opened yet they are read here, once. */
export async function settingRows() {
  if (!drawn.length) {
    const st = S.state || await loadState();
    const [b, ar] = await Promise.all([call('backups').catch(() => null), call('archive_status').catch(() => null)]);
    if (!drawn.length) drawn = sections(st, b, ar, null);
  }
  const t = document.createElement('template');
  return drawn.map(([id, title, body]) => {
    t.innerHTML = body;
    return { id, title, rows: [...t.content.querySelectorAll('.srow')].map((r) => ({ label: r.querySelector('.what b').textContent, help: r.querySelector('.what p').textContent })) };
  });
}
/** The two columns: the list of sections with the search on top (no section is lit while the search shows rows of several), and the right side. */
function columns() {
  const finding = !!query.trim(), lit = (id) => (!finding && id === chosen ? 'class="on" aria-current="true"' : '');
  return `<nav class="card set-nav" aria-label="Settings sections" data-testid="set-nav"><div>
      <label class="set-find"><svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" aria-hidden="true"><circle cx="7" cy="7" r="4"/><path d="M10 10l3.5 3.5"/></svg><input id="set-find" type="search" aria-label="Search settings" placeholder="Search settings" autocomplete="off" value="${esc(query)}" data-testid="set-find"></label>
      ${drawn.map(([id, title]) => `<a href="#settings?tab=${id}" ${lit(id)} data-act="set-tab" data-v="${id}" data-testid="set-tab-${id}">${title}</a>`).join('')}
    </div></nav>
    <div class="set-panes" id="set-panes" data-testid="set-panes">${panes()}</div>`;
}

view('settings', async (sheet, params) => {
  // Computers was a section here until 1.9: an old link to it opens its page
  if (params && params.tab === 'computers') return location.replace('#computers');
  sheet.className = 'sheet';
  const st = await loadState();
  // (the sync folder's state, for the backup rows)
  const [b, ar, path] = await Promise.all([call('backups').catch(() => null), call('archive_status').catch(() => null), call('path_status').catch(() => null), computersStatus().catch(() => null)]);
  drawn = sections(st, b, ar, path);
  // #settings?tab=updates (Apps › Settings › Updates): that section; an address without one, or with one this system lacks, opens the first
  chosen = drawn.some(([id]) => id === (params && params.tab)) ? params.tab : 'appearance';
  patch(sheet, `<div class="page-body b-page" data-b-page="settings">
    ${pageHead('settings', 'Saved as you change them. Keys and logins never enter these files or backups.')}
    <div class="set" id="set">${columns()}</div>
  </div>`);
});
// the search filters as it is typed, from the sections already read
document.addEventListener('input', (e) => {
  if (e.target.id !== 'set-find') return;
  query = e.target.value;
  patch(document.getElementById('set'), columns());
});
// a section chosen from the list ends the search; the address carries the section, so Back returns to the one before
action('set-tab', (el) => { query = document.getElementById('set-find').value = ''; if (el.dataset.v === chosen) render(); else go('settings', { tab: el.dataset.v }); });
window.addEventListener('hashchange', () => { if (!location.hash.startsWith('#settings')) query = ''; });

// ---------- Backups ----------
function backupsSection(b, ar, s) {
  const folderOn = !!(S.cache.computers && S.cache.computers.mode === 'folder');
  const days = s.archive_days ?? 30, loc = (ar && ar.locations) || [], shared = b.in_sync_folder || [];
  return row('data', 'Data folder', info('data', 'The account list (unless it lives elsewhere), settings, the usage database, backups and the log.<br><b>Default: this computer\'s app data.</b>'), 'The local usage index and shared setup.', `<span class="mono trunc" title="${esc(b.data)}">${esc(b.data)}</span>${btn('Open', 'open-folder', `data-path="${esc(b.data)}"`)}`)
    + row('bk', 'Automatic backups', info('bk', 'A backup holds the account list, settings, what Harness Manager wrote into each tool, the folders of the skills it manages, and the history: every request, session, limit reading and tag, with the computer each came from, and the archived sessions. API keys and saved logins stay in ' + esc(keyStore()) + ' and are never in a backup.<br><b>Default: every 24 hours, keeping 10.</b>'), 'Registry, usage and limits; keys and logins never enter backups.',
      `${pick('backup_hours', s.backup_hours, [[0, 'Off'], [6, '6 h'], [12, '12 h'], [24, '24 h'], [168, 'Week']], 'st-bkhours')}<label class="chk">Keep <select data-act="set-now" data-key="backup_keep" aria-label="Backups kept" data-testid="st-bkkeep">${[3, 5, 10, 20, 50].map((v) => `<option value="${v}" ${s.backup_keep === v ? 'selected' : ''}>${v}</option>`).join('')}</select></label>`)
    + row('bkfolder', 'Also in the sync folder', info('bkfolder', 'Every backup is also copied to <span class="mono">backups\\&lt;this computer&gt;</span> in the sync folder, keeping as many there as here, so a fresh computer finds its history beside the journal.<br><b>Default: on.</b>'), folderOn ? `Copy backups to ${esc(s.sync_folder)}.` : 'Once a sync folder is chosen.', sw('backup_in_sync_folder', s.backup_in_sync_folder !== false, 'Copy backups there', 'st-bk-folder'))
    + row('bktr', 'Include transcripts', info('bktr', 'The zip also holds this computer\'s session transcripts, compressed, so a restore brings the conversations back too. Backups get larger.<br><b>Default: off.</b>'), 'Include session transcripts in backups.', sw('backup_transcripts', !!s.backup_transcripts, 'Include transcripts', 'st-bk-transcripts'))
    + block('backups', 'Backups', `${b.list.length} · ${kb(b.list.reduce((a, x) => a + x.size, 0))}`, `${btn('Import…', 'backup-import', 'data-testid="backup-import"')}${btn('Export…', 'backup-export', 'data-testid="backup-export"')}${btn('Back up now', 'backup-now', 'data-testid="backup-now"')}`)
    + `<div class="set-list" data-key="list-backups">${b.list.map((x) => `<div class="set-item" data-key="backup-${esc(x.name)}" data-testid="backup-${esc(x.label)}"><div class="grow"><b>${esc(x.label)}</b><p>${esc(stamp(x.at))} · ${kb(x.size)}</p></div>${btn('Rename', 'backup-rename', `data-name="${esc(x.name)}" data-label="${esc(x.label)}"`)}${btn('Restore…', 'backup-restore', `data-name="${esc(x.name)}" data-testid="restore-${esc(x.label)}"`)}<button class="btn sm danger" data-act="backup-delete" data-name="${esc(x.name)}">Delete…</button></div>`).join('') || '<p class="set-none">No backups yet.</p>'}</div>`
    + block('folder-backups', 'In the sync folder', `${shared.length} · every computer's`, '')
    + `<div class="set-list" data-key="list-folder-backups">${shared.map((x) => `<div class="set-item" data-key="folder-backup-${esc(x.path)}" data-testid="folder-backup-${esc(x.label)}"><div class="grow"><b>${esc(x.label)} · ${esc(x.device_name)}</b><p>${esc(stamp(x.at))} · ${kb(x.size)}</p></div>${btn('Restore…', 'restore-src', `data-zip="${esc(x.path)}"`)}</div>`).join('') || '<p class="set-none">No shared backups.</p>'}</div>`
    + row('arch', 'Archive sessions older than', info('arch', 'Transcripts of sessions older than this move from the tools\' folders into the archive, compressed by project and month, once a day while the app runs and with Archive now. Each copy is checked before the original is deleted. The session stays in the list with its numbers and stays readable; it cannot be resumed from the archive. Usage is never touched.<br><b>Default: 30 days.</b> Never keeps every transcript where its tool put it.'), 'Archive old transcripts; keep the usage index.',
      `${pick('archive_days', days, [[0, 'Never'], [30, '30 days'], [90, '90 days'], [365, '1 year']], 'st-archive-days')}${field('st-archive-custom', 'Custom days', `class="mono" type="number" min="0" max="36500" value="${days}" data-testid="st-archive-custom"`)}${btn('Save', 'archive-days-custom', 'data-testid="st-archive-custom-save"')}`)
    + row('archloc', 'Archive location', info('archloc', 'Where the copies go: the sync folder (<span class="mono">archive\\&lt;this computer&gt;</span>, so every computer and a fresh install can read them), a folder of your choice, or both.<br><b>Default: the sync folder.</b> With neither, nothing is archived.'), 'Copy archived transcripts to these locations.',
      `${sw('archive_in_sync_folder', s.archive_in_sync_folder !== false, `Sync folder${folderOn ? '' : ' (none chosen yet)'}`, 'st-archive-sync')}<span class="${s.archive_dir ? 'mono trunc' : 'muted'}" title="${esc(s.archive_dir)}" data-testid="st-archive-dir">${esc(s.archive_dir || 'No folder chosen')}</span>${btn(s.archive_dir ? 'Change…' : 'Choose…', 'archive-dir-pick', 'data-testid="st-archive-dir-pick"')}${s.archive_dir ? btn('None', 'archive-dir-clear', 'data-testid="st-archive-dir-clear"') : ''}`)
    + block('archive', 'Archive', (ar ? `${ar.archived} archived · ${kb(ar.bytes)}${ar.last && ar.last.at ? ` · last run ${esc(when(ar.last.at))}` : ''}` : 'Nothing archived yet.') + (loc.length ? ' · ' + loc.map((l) => `${l.kind === 'sync_folder' ? 'sync folder' : 'folder'}: ${l.sessions} (${l.on ? 'on' : 'off'})`).join(' · ') : ''), btn('Archive now', 'archive-now', 'data-testid="archive-now"'))
    + block('restore', 'Restore', "Merge a backup or the sync folder's history into this computer.", btn('Restore…', 'restore-pick', 'data-testid="restore-open"'));
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
action('cli-path', async (el) => {
  el.disabled = true;
  try {
    if (await attempt(() => call('path_set', { add: el.dataset.add === 'true' }))) render();
  } finally { el.disabled = false; }
});
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
  // a box's state, a choice's value, or a dropdown's choice (numbers as numbers)
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

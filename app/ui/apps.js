// Apps, as the reference draws it: the finding, a warning bar per tool installed twice, a table of the installed
// tools and one of the rest (Tool, Installed, Newest in amber when newer, Installed by, Action, Show), a second copy as
// a sub-row, and the Harness Manager card. Installs and updates open in a terminal; the page watches the version after.

import { S, call, view, go, toast, attempt, mutate, confirmDialog, openMenu, openDialog, closeDialog, dlgHead, action, render, setAppUpdates, patch } from './core.js';
import { esc, hm, toolIcon } from './lib.js';

// "2.10.0" is newer than "2.9.3"
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); } return false; };
const KIND = { native: 'native', npm: 'npm', brew: 'Homebrew', winget: 'winget', scoop: 'Scoop', choco: 'Chocolatey', unknown: 'manual' };
const kindOf = (c, a) => (KIND[c.kind] || c.kind) + (c.kind === 'native' && a.auto_updates ? ' · updates itself' : '');
const panel = (title, body, testid) => `<section class="panel-card" data-testid="${testid}"><div class="panel-head"><h2 class="panel-title">${title}</h2></div>${body}</section>`;

view('apps', async (sheet) => {
  sheet.className = 'sheet';
  if (!S.cache.apps) patch(sheet, '<div class="page-body b-page"><header class="page-heading"><div><h1>Reading each tool\'s version…</h1></div></header></div>');
  const [d, me] = await Promise.all([call('apps'), call('self_latest').catch(() => null)]);
  const st = S.state || (await call('state'));
  const hidden = st.settings.hidden_tools || [];
  const latest = (d.latest && d.latest.versions) || {}, errors = (d.latest && d.latest.errors) || {};
  const installed = d.apps.filter((a) => a.path), missing = d.apps.filter((a) => !a.path);
  const upd = (a) => a.path && latest[a.id] && a.version && newer(latest[a.id], a.version);
  const updates = installed.filter(upd);
  // a tool installed more than once: the copy that runs is named, the rest offered for removal
  const warn = installed.filter((a) => a.copies.length > 1).map((a) => {
    const runs = a.copies.find((c) => c.runs), other = a.copies.find((c) => !c.runs);
    const same = runs.version && other.version && runs.version === other.version;
    return `<div class="notice-bar warn" role="alert" data-testid="app-twice-${a.id}"><span><b>${esc(a.label)} is installed ${a.copies.length} times.</b> Your terminals run the ${esc(kindOf(runs, a))} copy${runs.version ? ' (' + esc(runs.version) + ')' : ''}; the ${esc(kindOf(other, a))} copy in <span class="mono">${esc(other.path)}</span> ${same ? 'is the same version' : other.version ? 'is ' + esc(other.version) : 'could not say its version'}. Keep one.</span>
      <button class="btn-secondary" data-act="app-remove-copy" data-id="${a.id}" data-path="${esc(other.path)}" data-kind="${esc(kindOf(other, a))}" data-testid="app-remove-${a.id}">Remove the ${esc(kindOf(other, a).split(' ')[0])} copy…</button></div>`;
  }).join('');
  // one row per tool: where it is, its version, the newest one, how it was installed, the action and Show; other copies under it
  const row = (a) => {
    const l = latest[a.id], n = upd(a), runs = a.copies.find((c) => c.runs);
    const act = !a.path
      ? (a.install.length ? `<button class="btn-primary" data-act="app-install" data-id="${a.id}" data-testid="app-install-${a.id}">Install…</button>` : a.download ? `<button class="btn-secondary" data-act="app-site" data-url="${esc(a.download)}">Website</button>` : '')
      : a.update ? (n ? `<button class="btn-primary" data-act="app-update" data-id="${a.id}" data-label="${esc(a.label)}" data-to="${esc(l)}" title="${esc(a.update.label)}: ${esc(a.update.command)}" data-testid="app-update-${a.id}">Update to ${esc(l)}</button>` : `<span class="muted">${l ? 'Up to date' : 'Not checked'}</span><button class="btn-secondary" data-act="app-update" data-id="${a.id}" data-label="${esc(a.label)}" title="${esc(a.update.label)}: ${esc(a.update.command)}" data-testid="app-update-${a.id}">Update</button>`) : '';
    const newest = l ? `<span class="${n ? 'b-warn' : ''}" title="${n ? 'Newer than the installed version' : 'The newest version the vendor offers'}">${esc(l)}</span>` : errors[a.id] ? `<span class="muted" title="${esc(errors[a.id])}">could not check</span>` : '<span class="muted">—</span>';
    return `<tr data-key="app-${a.id}" data-testid="app-${a.id}"><td><div class="b-name">${toolIcon(a.id, 16)}<b>${esc(a.label)}</b></div><div class="mono b-command" title="${esc(a.path || '')}">${a.path ? esc(a.path) : a.install.length ? 'Not installed' : 'Not installed; get it from its website'}</div></td>
      <td>${a.path ? esc(a.version || '?') : '—'}</td><td>${newest}</td><td data-testid="app-kind-${a.id}">${runs ? esc(kindOf(runs, a)) : '—'}</td>
      <td><div class="page-actions">${act}${a.path ? `<button class="icon-btn" data-act="app-menu" data-id="${a.id}" data-path="${esc(a.path)}" aria-label="More for ${esc(a.label)}">⋯</button>` : ''}</div></td>
      <td><label class="b-switch"><input type="checkbox" data-act="app-show" data-id="${a.id}" ${hidden.includes(a.id) ? '' : 'checked'} aria-label="Show ${esc(a.label)} on Accounts, MCP servers, Skills and System prompts" data-testid="app-show-${a.id}"> Show</label></td></tr>
      ${a.copies.filter((c) => !c.runs).map((c) => `<tr class="b-copy-row" data-key="copy-${esc(c.path)}" data-testid="app-copy-${a.id}"><td colspan="4"><span class="mono">${esc(c.path)}</span><span class="muted"> · ${esc(c.version || '?')} · ${esc(kindOf(c, a))} · second copy, not on PATH first</span></td><td colspan="2"><button class="btn-secondary" data-act="app-remove-copy" data-id="${a.id}" data-path="${esc(c.path)}" data-kind="${esc(kindOf(c, a))}">Remove…</button></td></tr>`).join('')}`;
  };
  const table = (rows) => `<table class="replies-table b-apps"><thead><tr><th>Tool</th><th>Installed</th><th>Newest</th><th>Installed by</th><th>Action</th><th>Show</th></tr></thead><tbody>${rows.map(row).join('')}</tbody></table>`;
  const finding = updates.length ? `${updates.length} ${updates.length === 1 ? 'tool has' : 'tools have'} an update.` : !installed.length ? 'No tools installed yet.' : !d.latest ? 'Your tools are installed; the newest versions are not checked yet.' : 'Your tools are up to date.';
  patch(sheet, `<div class="page-body b-page" data-b-page="apps">
    <header class="page-heading"><div><h1>${finding}</h1><div class="heading-facts">${installed.length} installed · ${missing.length} not installed · checked ${d.latest ? esc(hm(d.latest.at)) : '—'}</div></div>
      <div class="page-actions"><button class="btn-secondary" data-act="apps-check" data-testid="apps-check">Check for updates</button></div></header>
    ${warn}
    ${panel(`Installed · ${installed.length}`, installed.length ? table(installed) : '<div class="b-empty"><b>None of the tools is installed yet.</b><span class="muted">Install one below.</span></div>', 'apps-installed')}
    ${missing.length ? panel(`Not installed · ${missing.length}`, table(missing), 'apps-missing') : ''}
    <section class="panel-card"><div class="panel-head"><h2 class="panel-title">Harness Manager</h2></div>
      <div class="b-toolbar"><span>${esc(st.version)} · ${me && me.version ? `${esc(me.version)} available` : me ? 'the newest' : 'not checked yet'}</span><span class="plan-pill ${me && me.version ? 'b-warn' : me ? 'b-good' : ''}">${me && me.version ? 'Update available' : me ? 'Up to date' : 'Not checked'}</span><button class="btn-secondary" data-act="go-settings-updates">Settings › Updates</button></div>
      <span class="muted">Checks at launch and every hour · signed updates only</span></section>
  </div>`);
  S.cache.apps = d.apps;
  if (d.updates != null) setAppUpdates(d.updates); // counted against the versions installed now: an applied update leaves it
});
action('go-settings-updates', () => go('settings', { tab: 'updates' }));

action('apps-check', async () => {
  toast('Asking each vendor for the newest versions…');
  const r = await attempt(() => call('apps_latest'));
  if (!r) return;
  setAppUpdates(r.updates);
  const bad = Object.keys(r.errors || {});
  if (bad.length) toast(`Could not check ${bad.join(', ')}: ${r.errors[bad[0]]}`, true);
  render();
});
// after a terminal runs an install or update, the page watches the tool's version for two minutes and redraws when it changes
function watchVersion(id, before) {
  const end = Date.now() + 120000;
  const t = setInterval(async () => {
    if (Date.now() > end || S.route.view !== 'apps') return clearInterval(t);
    const d = await call('apps').catch(() => null);
    const a = d && d.apps.find((x) => x.id === id);
    if (a && (a.version || null) !== before) { clearInterval(t); toast(a.path ? `${a.label} is now ${a.version || 'installed'}` : `${a.label} is uninstalled`); render(); }
  }, 5000);
}
const runIn = (cmd, id, before) => { if (cmd) { toast(`Running in a new terminal: ${cmd}`); watchVersion(id, before); } };
action('app-update', async (el) => {
  const id = el.dataset.id, a = (S.cache.apps || []).find((x) => x.id === id) || {};
  // a running tool can hold its own files open; say so before the update fails on them
  const open = ((S.state && S.state.profiles) || []).filter((p) => p.harness === id).reduce((n, p) => n + p.running.length, 0);
  if (open && !(await confirmDialog('Update ' + el.dataset.label, `Update ${esc(el.dataset.label)}${el.dataset.to ? ' to ' + esc(el.dataset.to) : ''}? ${open} ${open === 1 ? 'console is' : 'consoles are'} running it, and the update may not be able to replace files they use.`, 'Update', false))) return;
  runIn(await attempt(() => call('app_update', { id })), id, a.version || null);
});
action('app-install', (el) => {
  const a = (S.cache.apps || []).find((x) => x.id === el.dataset.id);
  if (!a) return;
  openDialog(`${dlgHead('Install ' + a.label)}${a.install.map((m) => `<div class="account-session"><div><b>${esc(m.label)}</b><div class="mono b-command">${esc(m.command)}</div></div><button class="btn-secondary" data-act="app-install-method" data-id="${a.id}" data-m="${esc(m.id)}">Use method</button></div>`).join('')}
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button></span></div>`, { size: 'sm', label: 'Install ' + a.label });
});
action('app-install-method', async (el) => { closeDialog(); runIn(await attempt(() => call('app_install', { id: el.dataset.id, method: el.dataset.m })), el.dataset.id, null); });
action('app-remove-copy', async (el) => {
  if (!(await confirmDialog('Remove this copy?', `The ${esc(el.dataset.kind)} copy at <span class="mono">${esc(el.dataset.path)}</span> is removed in a terminal window. The copy your terminals run stays.`, 'Remove'))) return;
  runIn(await attempt(() => call('app_remove_copy', { id: el.dataset.id, path: el.dataset.path })), el.dataset.id, '__copies__');
});
action('app-show', async (el) => {
  const st = S.state || (await call('state'));
  const hidden = new Set(st.settings.hidden_tools || []);
  if (el.checked) hidden.delete(el.dataset.id); else hidden.add(el.dataset.id);
  if (!(await mutate(() => call('settings_set', { key: 'hidden_tools', value: [...hidden] })))) el.checked = !el.checked;
});
action('app-site', (el) => attempt(() => call('open_url', { url: el.dataset.url })));
action('app-menu', (el) => {
  const a = (S.cache.apps || []).find((x) => x.id === el.dataset.id) || { install: [] };
  openMenu(el, [
    ...(a.update ? [{ label: a.update.label, run: () => document.querySelector(`[data-testid=app-update-${a.id}]`)?.click() }] : []),
    ...(a.install.filter((m) => m.id === 'native' && !(a.update && a.update.id === 'native')).map((m) => ({ label: 'Reinstall with the vendor\'s installer', run: async () => runIn(await attempt(() => call('app_install', { id: a.id, method: m.id })), a.id, a.version || null) }))),
    { label: 'Open its folder', run: () => attempt(() => call('open_folder', { path: el.dataset.path.replace(/[\\/][^\\/]+$/, '') })) },
    { label: 'Copy its path', run: async () => { await navigator.clipboard.writeText(el.dataset.path); toast('Copied'); } },
    ...(a.update ? [{ label: 'Copy the update command', run: async () => { await navigator.clipboard.writeText(a.update.command); toast('Copied: ' + a.update.command); } }] : []),
    { label: 'Uninstall…', danger: true, testid: `app-uninstall-${a.id}`, run: () => uninstall(a) },
  ]);
});
async function uninstall(a) {
  const open = ((S.state && S.state.profiles) || []).filter((p) => p.harness === a.id).reduce((n, p) => n + p.running.length, 0);
  const copies = a.copies || [];
  const body = `${copies.length > 1 ? `All ${copies.length} copies of ${esc(a.label)} are` : `${esc(a.label)} at <span class="mono">${esc(a.path)}</span> is`} removed in a terminal window, each the way it was installed. Its accounts, settings and sessions stay, so installing it again picks them up.${open ? ` <b>${open} ${open === 1 ? 'console is' : 'consoles are'} running it: close ${open === 1 ? 'it' : 'them'} first, or the files in use cannot be removed.</b>` : ''}`;
  if (!(await confirmDialog('Uninstall ' + a.label + '?', body, 'Uninstall'))) return;
  runIn(await attempt(() => call('app_uninstall', { id: a.id })), a.id, a.version || null);
}

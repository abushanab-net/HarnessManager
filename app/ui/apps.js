// Apps: the tools Harness Manager runs, every copy of each on this computer (the one terminals run first), how it
// was installed, its version and the newest one from its vendor (Check for updates), and installing, updating or
// removing a copy in a terminal window.

import { S, call, view, toast, attempt, mutate, confirmDialog, openMenu, action, render, setAppUpdates } from './core.js';
import { esc, ic, hm, toolIcon } from './lib.js';

// "2.10.0" is newer than "2.9.3"
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); } return false; };
const KIND = { native: 'native', npm: 'npm', brew: 'Homebrew', winget: 'winget', scoop: 'Scoop', choco: 'Chocolatey', unknown: 'manual' };
const kindOf = (c, a) => (KIND[c.kind] || c.kind) + (c.kind === 'native' && a.auto_updates ? ' · updates itself' : '');

view('apps', async (sheet) => {
  sheet.className = 'sheet';
  sheet.innerHTML = '<div class="trow"><div><div class="crumb">Apps</div><h1>Reading each tool\'s version…</h1></div></div>';
  const [d, me] = await Promise.all([call('apps'), call('self_latest').catch(() => null)]);
  const st = S.state || (await call('state'));
  const hidden = st.settings.hidden_tools || [];
  const latest = (d.latest && d.latest.versions) || {}, errors = (d.latest && d.latest.errors) || {};
  const installed = d.apps.filter((a) => a.path), missing = d.apps.filter((a) => !a.path);
  // a tool installed more than once: the copy that runs is named, the rest offered for removal
  const twice = installed.filter((a) => a.copies.length > 1);
  const warn = twice.map((a) => {
    const runs = a.copies.find((c) => c.runs), other = a.copies.find((c) => !c.runs);
    const same = runs.version && other.version && runs.version === other.version;
    return `<div class="bar2 warn" data-testid="app-twice-${a.id}">${esc(a.label)} is installed ${a.copies.length} times. Your terminals run the ${esc(kindOf(runs, a))} copy${runs.version ? ' (' + esc(runs.version) + ')' : ''}; the ${esc(kindOf(other, a))} copy in <span class="mono">${esc(other.path)}</span> ${same ? 'is the same version' : other.version ? 'is ' + esc(other.version) : 'could not say its version'}. Keep one.
      <button class="btn sm" data-act="app-remove-copy" data-id="${a.id}" data-path="${esc(other.path)}" data-kind="${esc(kindOf(other, a))}" data-testid="app-remove-${a.id}">Remove the ${esc(kindOf(other, a).split(' ')[0])} copy</button></div>`;
  }).join('');
  // one row per tool: where it is, its version, the newest one, how it was installed, and Show; other copies under it
  const row = (a) => {
    const l = latest[a.id], upd = a.path && l && a.version && newer(l, a.version);
    const runs = a.copies.find((c) => c.runs);
    const act = !a.path
      ? (a.install.length ? `<button class="btn wide primary" data-act="app-install" data-id="${a.id}" data-testid="app-install-${a.id}">Install ${ic.caret}</button>` : a.download ? `<button class="btn wide" data-act="app-site" data-url="${esc(a.download)}">Website</button>` : '')
      : a.update ? `<button class="btn wide ${upd ? 'primary' : ''}" data-act="app-update" data-id="${a.id}" data-label="${esc(a.label)}" title="${esc(a.update.label)}: ${esc(a.update.command)}" data-testid="app-update-${a.id}">${upd ? (a.update.id === 'native' ? 'Reinstall ' + esc(l) : 'Update') : l ? 'Up to date' : 'Update'}</button>` : '';
    const newest = l ? `<span class="mono ver ${upd ? 'warnT' : 'muted'}">${esc(l)}</span>` : errors[a.id] ? `<span class="mono ver muted" title="${esc(errors[a.id])}">could not check</span>` : '<span class="mono ver muted">—</span>';
    return `<div class="apr" data-testid="app-${a.id}"><div class="who">${toolIcon(a.id, 22)}<div><b>${esc(a.label)}</b><span class="mono" title="${esc(a.path || '')}">${a.path ? esc(a.path) : a.install.length ? 'Not installed' : 'Not installed; get it from its website'}</span></div></div>
      <span class="mono ver">${a.path ? esc(a.version || '?') : '—'}</span>
      ${newest}
      <span class="by" data-testid="app-kind-${a.id}">${runs ? esc(kindOf(runs, a)) : '—'}</span>
      <span class="go">${act}${a.path ? ` <button class="iconbtn sq" data-act="app-menu" data-id="${a.id}" data-path="${esc(a.path)}" aria-label="More for ${esc(a.label)}">${ic.dots}</button>` : ''}</span>
      <label class="chk" title="Show ${esc(a.label)} on Accounts, MCP servers, Skills and System prompts"><input type="checkbox" data-act="app-show" data-id="${a.id}" ${hidden.includes(a.id) ? '' : 'checked'} data-testid="app-show-${a.id}"> Show</label></div>
      ${a.copies.filter((c) => !c.runs).map((c) => `<div class="apr sub" data-testid="app-copy-${a.id}"><span></span><div class="who"><span class="mono" title="${esc(c.path)}">${esc(c.path)} · second copy, not on PATH first</span></div>
        <span class="mono ver">${esc(c.version || '?')}</span><span></span><span class="by">${esc(kindOf(c, a))}</span>
        <span class="go"><button class="btn sm" data-act="app-remove-copy" data-id="${a.id}" data-path="${esc(c.path)}" data-kind="${esc(kindOf(c, a))}">Remove…</button></span><span></span></div>`).join('')}`;
  };
  const head = '<div class="apr aph2"><span>Tool</span><span>Installed</span><span>Newest</span><span>Installed by</span><span></span><span>Show</span></div>';
  sheet.innerHTML = `<div class="crumb">Apps${d.latest ? ' · checked ' + hm(d.latest.at) : ''}</div><h1>The tools on this computer, every copy of each, and the newest version its vendor offers.</h1><p class="find">How each was installed; installs and updates open in a terminal so you can see what they do.</p>
    <div class="acts lead">${d.latest ? `<span class="muted sm">Checked ${hm(d.latest.at)}</span>` : ''}<button class="btn" data-act="apps-check" data-testid="apps-check">Check for updates</button></div>
    ${warn}
    <div class="apgroup" data-testid="apps-installed">${installed.length ? head + installed.map(row).join('') : '<p class="empty">None of the tools is installed yet: install one below.</p>'}</div>
    ${missing.length ? `<h2 class="sech">Not installed · ${missing.length}</h2><div class="apgroup" data-testid="apps-missing">${head}${missing.map(row).join('')}</div>` : ''}
    <section class="panel apme"><div><h2>Harness Manager</h2><p class="muted">${esc(st.version)}${me && me.version ? ` · ${esc(me.version)} is available: Settings › Updates installs it` : me ? ' · the newest' : ''}. It checks at launch and every hour, and installs signed updates only.</p></div>
      <span class="pill ${me && me.version ? 'warn' : 'teal'} mono">${me && me.version ? 'update available' : me ? 'up to date' : 'not checked yet'}</span></section>`;
  S.cache.apps = d.apps;
  if (d.updates != null) setAppUpdates(d.updates); // counted against the versions installed now: an applied update leaves it
});

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
    if (a && (a.version || null) !== before) { clearInterval(t); toast(`${a.label} is now ${a.version || 'installed'}`); render(); }
  }, 5000);
}
action('app-update', async (el) => {
  const id = el.dataset.id, a = (S.cache.apps || []).find((x) => x.id === id) || {};
  // a running tool can hold its own files open; say so before the update fails on them
  const open = ((S.state && S.state.profiles) || []).filter((p) => p.harness === id).reduce((n, p) => n + p.running.length, 0);
  if (open && !(await confirmDialog('Update ' + el.dataset.label, `${open} ${open === 1 ? 'console is' : 'consoles are'} running ${esc(el.dataset.label)}. The update may not be able to replace files they use. Update anyway?`, 'Update', false))) return;
  const cmd = await attempt(() => call('app_update', { id }));
  if (cmd) { toast(`Running in a new terminal: ${cmd}`); watchVersion(id, a.version || null); }
});
action('app-install', (el) => {
  const a = (S.cache.apps || []).find((x) => x.id === el.dataset.id);
  if (!a) return;
  openMenu(el, a.install.map((m) => ({ label: `${m.label}: ${m.command.length > 70 ? m.command.slice(0, 70) + '…' : m.command}`, run: async () => {
    const cmd = await attempt(() => call('app_install', { id: a.id, method: m.id }));
    if (cmd) { toast(`Running in a new terminal: ${cmd}`); watchVersion(a.id, null); }
  } })));
});
action('app-remove-copy', async (el) => {
  if (!(await confirmDialog('Remove this copy?', `The ${esc(el.dataset.kind)} copy at <span class="mono">${esc(el.dataset.path)}</span> is removed in a terminal window. The copy your terminals run stays.`, 'Remove'))) return;
  const cmd = await attempt(() => call('app_remove_copy', { id: el.dataset.id, path: el.dataset.path }));
  if (cmd) { toast(`Running in a new terminal: ${cmd}`); watchVersion(el.dataset.id, '__copies__'); }
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
    ...(a.install.filter((m) => m.id === 'native' && !(a.update && a.update.id === 'native')).map((m) => ({ label: 'Reinstall with the vendor\'s installer', run: async () => { const cmd = await attempt(() => call('app_install', { id: a.id, method: m.id })); if (cmd) { toast(`Running in a new terminal: ${cmd}`); watchVersion(a.id, a.version || null); } } }))),
    { label: 'Open its folder', run: () => attempt(() => call('open_folder', { path: el.dataset.path.replace(/[\\/][^\\/]+$/, '') })) },
    { label: 'Copy its path', run: async () => { await navigator.clipboard.writeText(el.dataset.path); toast('Copied'); } },
    ...(a.update ? [{ label: 'Copy the update command', run: async () => { await navigator.clipboard.writeText(a.update.command); toast('Copied: ' + a.update.command); } }] : []),
  ]);
});

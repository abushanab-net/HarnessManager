// Apps: the tools Harness Manager runs, where each is installed and its version, the newest version
// (Check for updates), and installing or updating one in a terminal window.

import { S, call, view, toast, attempt, confirmDialog, openMenu, action, render, setAppUpdates } from './core.js';
import { esc, ic, hm, toolIcon } from './lib.js';

// "2.10.0" is newer than "2.9.3"
const newer = (a, b) => { const x = a.split('.').map(Number), y = b.split('.').map(Number); for (let i = 0; i < Math.max(x.length, y.length); i++) { if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0); } return false; };

view('apps', async (sheet) => {
  sheet.className = 'sheet';
  sheet.innerHTML = '<div class="trow"><div><h1>Apps</h1><p class="find">Reading each tool\'s version…</p></div></div>';
  const [d, me] = await Promise.all([call('apps'), call('self_latest').catch(() => null)]);
  const st = S.state || (await call('state'));
  const hidden = st.settings.hidden_tools || [];
  const latest = (d.latest && d.latest.versions) || {};
  const installed = d.apps.filter((a) => a.path), missing = d.apps.filter((a) => !a.path);
  // one row per tool: where it is, its version, the newest one, what updates it, and Show
  const row = (a) => {
    const l = latest[a.id], upd = a.path && l && a.version && newer(l, a.version);
    const by = a.manager === 'npm' ? 'npm' : a.manager === 'self' ? 'updates itself' : a.download ? 'its website' : '—';
    const act = !a.path
      ? (a.manager === 'npm' ? `<button class="btn wide" data-act="app-update" data-id="${a.id}" data-label="${esc(a.label)}" data-testid="app-install-${a.id}">Install</button>`
        : a.download ? `<button class="btn wide" data-act="app-site" data-url="${esc(a.download)}">Website</button>` : '')
      : a.manager ? `<button class="btn wide ${upd ? 'primary' : ''}" data-act="app-update" data-id="${a.id}" data-label="${esc(a.label)}" data-testid="app-update-${a.id}">${upd ? 'Update' : a.manager === 'self' ? 'Run its updater' : 'Up to date'}</button>` : '';
    return `<div class="apr" data-testid="app-${a.id}"><div class="who">${toolIcon(a.id, 22)}<div><b>${esc(a.label)}</b><span class="mono">${a.path ? esc(a.path) : a.manager === 'npm' ? `Installs with npm (${esc(a.package)})` : 'Not installed'}</span></div></div>
      <span class="mono ver">${a.path ? esc(a.version || '?') : '—'}</span>
      <span class="mono ver ${upd ? 'warnT' : 'muted'}">${l ? esc(l) : '—'}</span>
      <span class="by">${by}</span>
      <span class="go">${act}${a.path ? ` <button class="iconbtn sq" data-act="app-menu" data-path="${esc(a.path)}" aria-label="More for ${esc(a.label)}">${ic.dots}</button>` : ''}</span>
      <label class="chk" title="Show ${esc(a.label)} on Accounts, MCP servers, Skills and Prompts"><input type="checkbox" data-act="app-show" data-id="${a.id}" ${hidden.includes(a.id) ? '' : 'checked'} data-testid="app-show-${a.id}"> Show</label></div>`;
  };
  const head = '<div class="apr aph2"><span>Tool</span><span>Installed</span><span>Newest</span><span>Updated by</span><span></span><span>Show</span></div>';
  sheet.innerHTML = `<h1>Apps</h1><p class="find">The tools on this computer, their version and the newest one. Updates open in a terminal so you can see what they do.</p>
    <div class="acts lead">${d.latest ? `<span class="muted sm">Checked ${hm(d.latest.at)}</span>` : ''}<button class="btn" data-act="apps-check" data-testid="apps-check">Check for updates</button></div>
    <div class="apgroup" data-testid="apps-installed">${head}${installed.map(row).join('') || '<p class="empty">None of the tools is installed.</p>'}</div>
    ${missing.length ? `<h2 class="sech">Not installed · ${missing.length}</h2><div class="apgroup" data-testid="apps-missing">${head}${missing.map(row).join('')}</div>` : ''}
    <section class="panel apme"><div><h2>Harness Manager</h2><p class="muted">${esc(st.version)}${me && me.version ? ` · ${esc(me.version)} is available: Settings › Updates installs it` : me ? ' · the newest' : ''}. It checks at launch and once a day, and installs signed updates only.</p></div>
      <span class="pill ${me && me.version ? 'warn' : 'teal'} mono">${me && me.version ? 'update available' : me ? 'up to date' : 'not checked yet'}</span></section>`;
});

action('apps-check', async () => {
  toast('Asking npm for the newest versions…');
  const r = await attempt(() => call('apps_latest'));
  if (r) { setAppUpdates(r.updates); render(); }
});
action('app-update', async (el) => {
  const id = el.dataset.id;
  // a running tool can hold its own files open; say so before the update fails on them
  const open = ((S.state && S.state.profiles) || []).filter((p) => p.harness === id).reduce((n, p) => n + p.running.length, 0);
  if (open && !(await confirmDialog('Update ' + el.dataset.label, `${open} ${open === 1 ? 'console is' : 'consoles are'} running ${esc(el.dataset.label)}. The update may not be able to replace files they use. Update anyway?`, 'Update', false))) return;
  const cmd = await attempt(() => call('app_update', { id }));
  if (cmd) toast(`Running ${cmd} in a new terminal; this page shows the new version once it finishes and you open it again`);
});
action('app-show', async (el) => {
  const st = S.state || (await call('state'));
  const hidden = new Set(st.settings.hidden_tools || []);
  if (el.checked) hidden.delete(el.dataset.id); else hidden.add(el.dataset.id);
  if (await attempt(() => call('settings_set', { key: 'hidden_tools', value: [...hidden] }))) { S.state = null; await call('state').then((x) => (S.state = x)); }
  else el.checked = !el.checked;
});
action('app-site', (el) => attempt(() => call('open_url', { url: el.dataset.url })));
action('app-menu', (el) => openMenu(el, [
  { label: 'Open its folder', run: () => attempt(() => call('open_folder', { path: el.dataset.path.replace(/[\\/][^\\/]+$/, '') })) },
  { label: 'Copy its path', run: async () => { await navigator.clipboard.writeText(el.dataset.path); toast('Copied'); } },
]));

// Skills, as the reference draws them: the finding, Installed / Discover tabs, the store path with a source filter,
// the "Linked tools" matrix (a header tick per tool links every skill shown), the Not managed panel, Discover with its
// repositories and "Install into" chips, the Install and Import dialogs. The page draws from its last data.

import { S, call, view, toast, attempt, mutate, changed, confirmDialog, openDialog, closeDialog, dlgHead, openMenu, action, render, shownTools, setNavCounts, patch, pickFolder } from './core.js';
import { esc, toolIcon, ic } from './lib.js';
import { syncReport, head, badge, empty, panel, field, dlgError, dlgFoot, pagerHtml, toolHead, toolCell, findText } from './mcp.js';

const sk = { tab: 'installed', source: '', updates: {}, disc: null, loading: false, per: 25, page: 0, dq: '', repo: '' };
const sourceOf = (s) => (s.source.startsWith('https://github.com/') ? s.source.replace('https://github.com/', '').split('/').slice(0, 2).join('/') : s.source === 'imported' ? 'Imported' : s.source === 'store' ? 'Store' : 'Local');
const onFor = (d, name, h) => d.targets.some((t) => t.kind === 'skill' && t.item === name && t.harness === h && !t.profile);
const canUpdate = (source) => source.startsWith('https://github.com/') || /^[a-zA-Z]:\\|^\//.test(source);
// the tools a skill from Discover is linked into: the last choice, Claude Code to begin with
const enableKey = 'hm.skill.enable';
const enabled = () => { try { return JSON.parse(localStorage.getItem(enableKey)) || ['claude']; } catch { return ['claude']; } };
const tabs = (total) => `<nav class="page-tabs" aria-label="Skills tabs">${[['installed', `Installed ${total}`], ['discover', 'Discover']].map(([t, l]) => `<button class="filter-chip ${sk.tab === t ? 'active' : ''}" data-act="sk-tab" data-v="${t}" aria-current="${sk.tab === t ? 'page' : 'false'}" data-testid="sk-tab-${t}">${l}</button>`).join('')}</nav>`;

let cache = null;
/** The rows the search and the source filter leave; the header ticks act on these. */
const filtered = (d) => { const q = findText(); return d.skills.filter((s) => (!q || (s.name + ' ' + ((d.skill_info[s.name] || {}).description || '') + ' ' + sourceOf(s)).toLowerCase().includes(q)) && (!sk.source || sourceOf(s) === sk.source)); };

function draw() {
  const { d, hs } = cache, unm = d.unmanaged, total = d.skills.length + unm.length, upd = Object.values(sk.updates).filter((v) => v === true).length;
  const h1 = unm.length ? `${unm.length} ${unm.length === 1 ? 'folder' : 'folders'} in the store ${unm.length === 1 ? 'is' : 'are'} not managed yet.` : total ? `${total} ${total === 1 ? 'skill shares' : 'skills share'} one store.` : 'No skills yet.';
  const top = head(h1, `${total} in store · ${d.skills.length} managed · ${unm.length} not managed · ${upd} ${upd === 1 ? 'update' : 'updates'}`,
    (sk.tab === 'discover' ? `<button class="btn-secondary" data-act="disc-refresh" data-testid="disc-refresh">Refresh</button>` : `<button class="btn-secondary" data-act="skill-check" data-testid="skill-check">Check for updates</button><button class="btn-secondary" data-act="import-from" data-kind="skill">Import from a tool</button>`)
    + `<button class="btn-primary" data-act="skill-install" data-testid="skill-install">Install skill</button>`) + tabs(d.skills.length);
  if (sk.tab === 'discover') return patch(document.getElementById('sheet'), `<div class="page-body b-page" data-b-page="skills">${top}${discover(d, hs)}</div>`);
  const list = filtered(d), pg = pagerHtml(list.length, sk, 'sk');
  const rows = list.slice(sk.page * sk.per, (sk.page + 1) * sk.per).map((s) => {
    const desc = (d.skill_info[s.name] || {}).description || '';
    return `<tr data-key="skill-${esc(s.name)}" data-name="${esc(s.name)}" data-testid="skill-row-${esc(s.name)}">
      <td><div class="b-name"><b>${esc(s.name)}</b>${sk.updates[s.name] === true ? `<button class="btn-secondary" data-act="skill-update" data-name="${esc(s.name)}" title="A newer version is at its source" data-testid="skill-update-${esc(s.name)}">Update</button>` : ''}</div>
        ${desc ? `<div class="muted b-clamp" title="${esc(desc)}">${esc(desc)}</div>` : ''}</td>
      <td title="${esc(s.source)}">${esc(sourceOf(s))}</td>
      ${hs.map((h) => toolCell('skill', s.name, h, onFor(d, s.name, h.id))).join('')}
      <td class="b-actions-cell"><button class="icon-btn" data-act="skill-menu" data-name="${esc(s.name)}" data-source="${esc(s.source)}" data-path="${esc(d.skill_store + '\\' + s.name)}" aria-label="More for ${esc(s.name)}" aria-haspopup="menu">${ic.dots}</button></td></tr>`;
  }).join('');
  // the header: each tool's tick stands for every skill shown; some ticked shows as indeterminate (set after the draw)
  const thead = `<thead><tr><th>Skill</th><th>Source</th>${hs.map((h) => { const n = list.filter((s) => onFor(d, s.name, h.id)).length; return toolHead(h, n, `<label>${n}<input type="checkbox" data-act="skill-bulk" data-h="${h.id}" data-bulk="${n > 0 && n < list.length ? 'mixed' : ''}" ${n === list.length && n ? 'checked' : ''} aria-label="Every skill shown, for ${esc(h.label)}" data-testid="skill-bulk-${h.id}"></label>`); }).join('')}<th><span class="b-sr">Actions</span></th></tr></thead>`;
  const table = `<div class="b-matrix-wrap"><table class="replies-table b-matrix" data-testid="skill-matrix">${thead}<tbody>${rows || `<tr><td colspan="${hs.length + 3}">${empty('No skill matches.')}</td></tr>`}</tbody></table></div>${pg}`;
  const unmRows = unm.filter((u) => { const q = findText(); return !q || (u.name + ' ' + (u.description || '')).toLowerCase().includes(q); }).map((u) => `<div class="account-session" data-key="unmanaged-${esc(u.name)}" data-testid="skill-row-${esc(u.name)}"><div><b>${esc(u.name)}</b><div class="muted">${esc(u.description || '')}${u.description ? ' · ' : ''}${u.linked.length ? 'linked in ' + esc(u.linked.map((id) => (hs.find((h) => h.id === id) || { label: id }).label).join(', ')) : 'not linked'}</div></div><button class="btn-secondary" data-act="skill-adopt" data-name="${esc(u.name)}" data-testid="skill-adopt-${esc(u.name)}">Manage</button></div>`).join('');
  patch(document.getElementById('sheet'), `<div class="page-body b-page" data-b-page="skills">${top}
    <div class="b-toolbar"><span class="mono">${esc(d.skill_store)}</span><label>Source <select data-act="sk-source" id="sk-filter" aria-label="Skill source">${['', ...new Set(d.skills.map(sourceOf))].sort().concat(unm.length ? ['Not managed'] : []).map((v) => `<option value="${esc(v)}" ${sk.source === v ? 'selected' : ''}>${esc(v || 'All sources')}</option>`).join('')}</select></label><button class="btn-secondary" data-act="sk-jump" data-testid="skill-jump-unmanaged">Not managed · ${unm.length}</button>${sk.source === 'Not managed' || !d.skills.length ? '' : pg}</div>
    ${sk.source === 'Not managed' ? '' : d.skills.length ? panel('Linked tools', table) : panel('Installed', empty('No skills yet.', 'Find some under Discover, install one from a folder or a GitHub link, or import a tool\'s own skills.'))}
    ${unm.length && (!sk.source || sk.source === 'Not managed') ? panel(`Not managed · ${unm.length}`, unmRows, `<button class="btn-primary" data-act="skill-adopt" data-testid="skill-adopt-all">Manage all</button>`, 'id="sk-unmanaged"') : ''}</div>`);
  document.querySelectorAll('[data-bulk]').forEach((x) => { x.indeterminate = x.dataset.bulk === 'mixed'; });
}

function discover(d, hs) {
  const repos = d.skill_repos, on = enabled(), q = sk.dq.trim().toLowerCase();
  const groups = sk.disc ? (sk.disc.filter((r) => repos.some((x) => x.owner + '/' + x.name === r.repo))).map((r) => {
    const hits = r.skills.filter((s) => !q || (s.name + ' ' + (s.description || '') + ' ' + r.repo).toLowerCase().includes(q));
    return panel(esc(r.repo), r.error ? `<span class="b-danger">${esc(r.error)}</span>` : hits.map((s) => `<div class="account-session" data-key="dc-${esc(s.folder)}" data-testid="dc-${esc(s.folder)}"><div><b>${esc(s.name)}</b> ${s.path ? `<span class="mono muted">${esc(s.path)}</span>` : ''}${s.description ? `<div class="muted">${esc(s.description)}</div>` : ''}</div>${s.installed ? badge('Installed', 'good') : `<button class="btn-secondary" data-act="dc-install" data-url="${esc(s.url)}" data-folder="${esc(s.folder)}" data-testid="dc-install-${esc(s.folder)}">Install</button>`}</div>`).join('')
      || empty(r.skills.length ? 'No skill matches.' : 'No SKILL.md in this repository.'), '', `data-key="repo-${esc(r.repo)}"`);
  }).join('') : empty(`Reading ${repos.length} ${repos.length === 1 ? 'repository' : 'repositories'}…`, 'the first time downloads each one.');
  return `<div class="b-toolbar">${field('dc-q', 'Search skills in these repositories', sk.dq, { extra: 'data-f="dq"', testid: 'dc-search' })}${field('dc-repo', 'Repository', sk.repo, { extra: 'placeholder="owner/repository or a GitHub link" data-f="repo"', testid: 'dc-repo' })}<button class="btn-secondary" data-act="repo-add" data-testid="dc-repo-add">Add</button></div>
    <div class="chip-group">${repos.map((r) => `<button class="filter-chip" data-act="repo-remove" data-o="${esc(r.owner)}" data-n="${esc(r.name)}" aria-label="Remove ${esc(r.owner + '/' + r.name)}" title="Remove this repository">${esc(r.owner + '/' + r.name)} ×</button>`).join('')}</div>
    <div class="b-toolbar"><b>Install into</b>${hs.map((h) => `<label class="filter-chip"><input type="checkbox" data-act="dc-enable" data-h="${h.id}" ${on.includes(h.id) ? 'checked' : ''} data-testid="dc-for-${h.id}"> ${toolIcon(h.id, 14)} ${esc(h.label)}</label>`).join('')}<span class="muted">Remembered for the next install; Installed ticks the rest</span></div>
    <div data-testid="discover">${groups}</div>`;
}

view('skills', async (sheet) => {
  sheet.className = 'sheet';
  const d = await call('items');
  setNavCounts({ 'n-skills': d.skills.length });
  const hs = shownTools(S.state || (await call('state'))).filter((h) => h.caps.skills);
  cache = { d, hs };
  S.pageFind = () => { sk.page = 0; draw(); };
  draw();
  // fetched after the first draw, then drawn again
  if (sk.tab === 'discover' && !sk.disc && !sk.loading) {
    sk.loading = true;
    attempt(() => call('discover')).then((r) => { sk.disc = r || []; sk.loading = false; render(); });
  }
});
// Discover's own search and repository fields keep their text in the page's state, so a redraw keeps them
document.addEventListener('input', (e) => { const f = e.target.dataset && e.target.dataset.f; if (f === 'dq' || f === 'repo') { sk[f] = e.target.value; if (f === 'dq' && cache) draw(); } });

action('dc-enable', () => { localStorage.setItem(enableKey, JSON.stringify([...document.querySelectorAll('[data-act=dc-enable]:checked')].map((i) => i.dataset.h))); });
action('sk-tab', (el) => { sk.tab = el.dataset.v; sk.page = 0; render(); });
action('sk-source', (el) => { sk.source = el.value; sk.page = 0; draw(); });
action('sk-jump', () => document.getElementById('sk-unmanaged')?.scrollIntoView({ block: 'start' }));
action('sk-page', (el) => { sk.page = +el.dataset.v; draw(); document.querySelector('.b-matrix-wrap')?.scrollTo(0, 0); });
action('sk-per', (el) => { sk.per = +el.value; sk.page = 0; draw(); });
action('skill-adopt', async (el) => {
  const names = el.dataset.name ? [el.dataset.name] : [];
  const r = await mutate(() => call('skill_adopt', { names }));
  if (!r) return;
  toast(`Now managed here: ${r.adopted.length === 1 ? r.adopted[0] : r.adopted.length + ' skills'}`);
  if (r.sync && r.sync.error) toast('Managed, but linking into the tools failed: ' + r.sync.error, true);
});
// a tool's header tick: every skill shown, on or off, in one write; a big change asks first
action('skill-bulk', async (el) => {
  const items = filtered(cache.d).map((s) => s.name), on = el.checked, tool = (cache.hs.find((h) => h.id === el.dataset.h) || { label: el.dataset.h }).label;
  if (items.length > 10 && !(await confirmDialog(`${on ? 'Link' : 'Unlink'} ${items.length} skills`, `${on ? 'Link' : 'Unlink'} every skill shown, ${items.length} of them, ${on ? 'into' : 'from'} <b>${esc(tool)}</b> in one write?`, on ? 'Link' : 'Unlink', !on))) { el.checked = !on; return; }
  const rep = await mutate(() => call('target_bulk', { kind: 'skill', harness: el.dataset.h, items, on }), `${on ? 'Linked' : 'Unlinked'} ${items.length} ${items.length === 1 ? 'skill' : 'skills'} ${on ? 'into' : 'from'} ${tool}`);
  syncReport(rep);
});
action('disc-refresh', () => {
  sk.disc = null; sk.loading = true; render();
  attempt(() => call('discover', { refresh: true })).then((r) => { sk.disc = r || []; sk.loading = false; render(); });
});
action('repo-add', async () => {
  const v = sk.repo.trim();
  if (!v) return toast('Enter owner/repository or a GitHub link', true);
  if (await attempt(() => call('skill_repo_add', { repo: v }), 'Repository added')) { sk.repo = ''; sk.disc = null; changed(); }
});
action('repo-remove', async (el) => { if (await attempt(() => call('skill_repo_remove', { owner: el.dataset.o, name: el.dataset.n }))) { sk.disc = null; changed(); } });
// installed into the store and linked into the tools ticked under Install into
action('dc-install', async (el) => {
  el.disabled = true;
  const enable = enabled();
  const r = await attempt(() => call('skill_install', { source: el.dataset.url, name: el.dataset.folder, enable }));
  if (!r) { el.disabled = false; return; }
  toast(`Installed ${r.name}${enable.length ? ' for ' + enable.join(', ') : ''}; Installed ticks the other tools`);
  // every repository that lists a skill of that name shows it installed
  for (const g of sk.disc || []) for (const s of g.skills) if (s.folder === r.name) s.installed = true;
  syncReport(r.sync, r.name);
  changed();
});
action('skill-check', async () => {
  toast('Checking each skill\'s source…');
  const r = await attempt(() => call('skill_check'));
  if (!r) return;
  sk.updates = Object.fromEntries(r.map((x) => [x.name, x.update]));
  const u = r.filter((x) => x.update === true).length;
  toast(u ? `${u} ${u === 1 ? 'update' : 'updates'} available: use Update beside the skill` : 'Every skill with a source to check is up to date');
  render();
});
action('skill-update', async (el) => {
  if (await mutate(() => call('skill_update', { name: el.dataset.name }), `Updated ${el.dataset.name}`)) sk.updates[el.dataset.name] = false;
});
action('skill-menu', (el) => openMenu(el, [
  ...(canUpdate(el.dataset.source) ? [{ label: 'Update from its source', run: async () => { if (await mutate(() => call('skill_update', { name: el.dataset.name }), `Updated ${el.dataset.name}`)) sk.updates[el.dataset.name] = false; } }] : []),
  { label: 'Open folder', run: () => attempt(() => call('open_folder', { path: el.dataset.path })) },
  { label: 'Remove…', danger: true, run: async () => {
    if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from every tool it is linked into?`, 'Remove')) {
      syncReport(await mutate(() => call('item_remove', { kind: 'skill', name: el.dataset.name })));
    }
  } },
]));

// ---------- the dialogs: an error stays inside the dialog, under its fields ----------
const showErr = (card, msg) => { let el = card.querySelector('#dlg-err'); if (!el) { el = document.createElement('div'); el.className = 'dlg-text err'; el.id = 'dlg-err'; el.setAttribute('role', 'alert'); card.querySelector('.modal-foot').before(el); } el.textContent = msg; };
action('import-from', async () => {
  const hs = shownTools(S.state || await call('state')).filter((h) => h.caps.skills);
  const card = openDialog(`${dlgHead('Import skills')}<div class="b-dialog-content">
    <div class="form-field"><label for="im-h">From</label><select id="im-h">${hs.map((h) => `<option value="${h.id}">${esc(h.label)} (main profile)</option>`).join('')}</select></div>
    <span class="muted">Its skill folders move to the shared store and are linked back.</span></div>${dlgFoot('Import', 'im-ok')}`, { label: 'Import skills', size: 'sm' });
  action('im-ok', async () => {
    let r;
    try { r = await call('skill_import', { harness: document.getElementById('im-h').value }); } catch (e) { return showErr(card, String(e).replace(/^Error: /, '')); }
    const added = Array.isArray(r) ? r : r.added, kept = r.kept || [];
    closeDialog();
    toast(`Imported ${added.length}: ${added.join(', ') || 'nothing new'}${kept.length ? '. Kept your own ' + kept.join(', ') + ': a different one is installed' : ''}`);
    changed();
  });
});
action('skill-install', () => {
  const card = openDialog(`${dlgHead('Install skill')}<div class="b-dialog-content">
    <div class="form-field"><label for="sk-src">Source *</label><div class="field-row"><input id="sk-src" placeholder="GitHub tree link or folder" data-testid="sk-src"><button class="btn-secondary" data-act="sk-pick">Folder…</button></div></div>
    ${field('sk-name', 'Name (optional)', '', { extra: 'placeholder="The folder name otherwise"' })}
    <span class="muted">Installed once in the store; tick the tools it should be in.</span></div>${dlgFoot('Install skill', 'sk-ok', 'sk-save')}`, { label: 'Install skill', size: 'sm' });
  action('sk-pick', async () => { const p = await pickFolder(); if (p) document.getElementById('sk-src').value = p; });
  action('sk-ok', async () => {
    const src = document.getElementById('sk-src').value.trim(), name = document.getElementById('sk-name').value.trim() || src.split(/[\\/]/).filter(Boolean).pop() || '';
    const err = !src ? 'Enter a GitHub tree link or a folder.' : /^https?:/i.test(src) && !/^https:\/\/github\.com\/[^/]+\/[^/]+\/tree\//.test(src) ? 'Use a GitHub tree link to a skill folder.'
      : !/^https:\/\/github\.com\//.test(src) && !/^([a-z]:[\\/]|\/|~[\\/])/i.test(src) ? 'Enter an absolute folder path.' : !/^[\w.-]+$/.test(name) ? 'Use a folder name without spaces or slashes.' : '';
    if (err) return showErr(card, err);
    let r;
    try { r = await call('skill_install', { source: src, name: document.getElementById('sk-name').value.trim() }); } catch (e) { return showErr(card, String(e).replace(/^Error: /, '')); }
    closeDialog();
    toast(`Installed ${r.name}; tick the tools it should be in`);
    syncReport(r.sync, r.name);
    changed();
  });
});

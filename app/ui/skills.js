// Skills: Installed (search, source filter, a tick per tool with a header tick for the whole column, updates) and
// Discover (skills in GitHub repositories, installed into Harness Manager's one store and linked into the tools you pick).

import { S, call, view, toast, attempt, mutate, changed, confirmDialog, openMenu, action, render, shownTools, setNavCounts } from './core.js';
import { esc, ic } from './lib.js';
import { syncReport, headState } from './views.js';

const sk = { tab: 'installed', source: '', updates: {}, disc: null, loading: false };
const sourceOf = (s) => (s.source.startsWith('https://github.com/') ? s.source.replace('https://github.com/', '').split('/').slice(0, 2).join('/') : s.source === 'imported' ? 'Imported' : s.source === 'store' ? 'Store' : 'Local');
const onFor = (d, name, h) => d.targets.some((t) => t.kind === 'skill' && t.item === name && t.harness === h && !t.profile);
const canUpdate = (s) => s.source.startsWith('https://github.com/') || /^[a-zA-Z]:\\|^\//.test(s.source);
// the tools a skill from Discover is linked into: the last choice, Claude Code to begin with
const enableKey = 'hm.skill.enable';
const enabled = () => { try { return JSON.parse(localStorage.getItem(enableKey)) || ['claude']; } catch { return ['claude']; } };

view('skills', async (sheet) => {
  sheet.className = 'sheet';
  const d = await call('items');
  setNavCounts({ 'n-skills': d.skills.length });
  const hs = shownTools(S.state || (await call('state'))).filter((h) => h.caps.skills);
  const sources = [...new Set(d.skills.map(sourceOf))].sort();
  const head = `<div class="trow"><div><h1>Skills</h1><p class="find">Skills live once, in <span class="mono">${esc(d.skill_store)}</span>, and are linked into each tool you tick, so an update reaches all of them.${d.unmanaged.length ? ` <b>${d.unmanaged.length} ${d.unmanaged.length === 1 ? 'folder' : 'folders'} there ${d.unmanaged.length === 1 ? 'is' : 'are'} not managed yet</b>: Manage adds ${d.unmanaged.length === 1 ? 'it' : 'them'}, ticked for the tools that already link to ${d.unmanaged.length === 1 ? 'it' : 'them'}.` : ''}</p></div>
      <div class="acts">${sk.tab === 'installed' ? `<button class="btn outline" data-act="skill-check" data-testid="skill-check">Check for updates</button><button class="btn outline" data-act="import-from" data-kind="skill">Import from a tool</button>` : `<button class="btn outline" data-act="disc-refresh" data-testid="disc-refresh">${ic.refresh} Refresh</button>`}<button class="btn primary" data-act="skill-install" data-testid="skill-install">${ic.plus} Install skill</button></div></div>
    <div class="tabrow"><div class="tabs2"><a class="${sk.tab === 'installed' ? 'on' : ''}" data-act="sk-tab" data-v="installed" data-testid="sk-tab-installed">Installed <span class="muted">${d.skills.length + d.unmanaged.length}</span></a><a class="${sk.tab === 'discover' ? 'on' : ''}" data-act="sk-tab" data-v="discover" data-testid="sk-tab-discover">Discover</a></div>
    ${sk.tab === 'installed' ? `<span class="muted sm sksum">Source: <select class="bare" id="sk-filter" aria-label="Source"><option value="">all</option>${[...new Set(d.skills.map(sourceOf)), ...(d.unmanaged.length ? ['Not managed'] : [])].sort().map((x) => `<option ${x === sk.source ? 'selected' : ''}>${esc(x)}</option>`).join('')}</select> · ${d.skills.length} managed · ${d.unmanaged.length} not managed${Object.values(sk.updates).filter((v) => v === true).length ? ' · ' + Object.values(sk.updates).filter((v) => v === true).length + ' updates' : ''}</span>` : ''}</div>`;
  if (sk.tab === 'discover') return discover(sheet, head, d, hs);
  const rows = d.skills.map((s) => {
    const u = sk.updates[s.name], desc = (d.skill_info[s.name] || {}).description || '';
    return `<tr data-search="${esc((s.name + ' ' + desc + ' ' + sourceOf(s)).toLowerCase())}" data-source="${esc(sourceOf(s))}" data-name="${esc(s.name)}" data-testid="skill-row-${esc(s.name)}">
      <td><div class="mn"><b>${esc(s.name)}</b>${u === true ? `<button class="tag upd" data-act="skill-update" data-name="${esc(s.name)}" title="A newer version is at its source" data-testid="skill-update-${esc(s.name)}">${ic.download} Update</button>` : ''}</div>
        ${desc ? `<div class="md clamp" title="${esc(desc)}">${esc(desc)}</div>` : ''}</td>
      <td class="mono src" title="${esc(s.source)}">${esc(sourceOf(s))}</td>
      ${hs.map((h) => `<td class="c"><input type="checkbox" data-act="toggle-target" data-kind="skill" data-item="${esc(s.name)}" data-h="${h.id}" ${onFor(d, s.name, h.id) ? 'checked' : ''} aria-label="${esc(s.name)} for ${esc(h.label)}" data-testid="t-skill-${esc(s.name)}-${h.id}"></td>`).join('')}
      <td class="num"><button class="iconbtn sq" data-act="skill-menu" data-name="${esc(s.name)}" data-source="${esc(s.source)}" data-path="${esc(d.skill_store + '\\' + s.name)}" aria-label="More for ${esc(s.name)}">${ic.dots}</button></td></tr>`;
  }).join('');
  const unm = d.unmanaged.map((u) => `<tr class="unm" data-search="${esc((u.name + ' ' + (u.description || '')).toLowerCase())}" data-source="Not managed" data-testid="skill-row-${esc(u.name)}">
      <td><div class="mn"><b>${esc(u.name)}</b></div>
        ${u.description ? `<div class="md clamp" title="${esc(u.description)}">${esc(u.description)}</div>` : ''}</td>
      <td class="mono src">${u.linked.length ? 'linked in ' + esc(u.linked.map((id) => (hs.find((h) => h.id === id) || { label: id }).label).join(', ')) : 'not linked'}</td>
      <td class="num" colspan="${hs.length + 1}"><button class="btn sm" data-act="skill-adopt" data-name="${esc(u.name)}" data-testid="skill-adopt-${esc(u.name)}">Manage</button></td></tr>`).join('');
  const total = d.skills.length + d.unmanaged.length;
  if (d.unmanaged.length && !sources.includes('Not managed')) sources.push('Not managed');
  // the header: each tool's tick stands for every skill shown; some ticked shows as indeterminate
  const thead = `<thead><tr><th>Skill</th><th>Source</th>${hs.map((h) => `<th class="c" data-label="${esc(h.label)}" title="${esc(h.label)}: tick every skill shown"><label class="ti">${esc(h.label.replace(/ Code$/, ''))}<small></small><input type="checkbox" data-act="skill-bulk" data-h="${h.id}" aria-label="Every skill shown, for ${esc(h.label)}" data-testid="skill-bulk-${h.id}"></label></th>`).join('')}<th></th></tr></thead>`;
  sheet.innerHTML = `${head}
    <div class="tcard">
    ${total ? `<table class="grid mlist sklist" data-testid="skill-matrix">${thead}<tbody>${rows}<tr class="nomatch" hidden><td colspan="${hs.length + 3}" class="empty">No skill matches.</td></tr></tbody></table>`
    : '<p class="empty">No skills yet. Find some under Discover, install one from a folder or a GitHub link, or import a tool\'s own skills.</p>'}</div>
    ${d.unmanaged.length ? `<section class="panel unmcard"><div class="ph"><h2>Not managed <span class="mono muted sm">${d.unmanaged.length} ${d.unmanaged.length === 1 ? 'folder' : 'folders'} in the store this list did not know</span></h2><button class="btn primary sm" data-act="skill-adopt" data-testid="skill-adopt-all">Manage all</button></div>
      <table class="grid sklist"><tbody>${unm}</tbody></table></section>` : ''}`;
  const find = document.getElementById('find'), src = document.getElementById('sk-filter'), table = sheet.querySelector('.sklist');
  const filter = () => {
    const t = find.value.trim().toLowerCase();
    let n = 0;
    sheet.querySelectorAll('tr[data-search]').forEach((tr) => { tr.hidden = (t && !tr.dataset.search.includes(t)) || (sk.source && tr.dataset.source !== sk.source); if (!tr.hidden) n++; });
    const nm = sheet.querySelector('tr.nomatch');
    if (nm) nm.hidden = n > 0;
    if (table) headState(table);
  };
  S.pageFind = filter;
  src.onchange = () => { sk.source = src.value; filter(); };
  filter();
});

async function discover(sheet, head, d, hs) {
  const repos = d.skill_repos, on = enabled();
  sheet.innerHTML = `${head}
    <div class="lbar"><span class="lsearch">${ic.search}<input class="input" id="dc-q" placeholder="Search skills in these repositories" aria-label="Search Discover" data-testid="dc-search"></span>
      <span class="repos">${repos.map((r) => `<span class="tag repo">${esc(r.owner + '/' + r.name)}<button data-act="repo-remove" data-o="${esc(r.owner)}" data-n="${esc(r.name)}" aria-label="Remove ${esc(r.owner + '/' + r.name)}" title="Remove this repository">${ic.x}</button></span>`).join('')}</span>
      <span class="addrepo"><input class="input" id="dc-repo" placeholder="owner/repository or a GitHub link" aria-label="Add a repository" data-testid="dc-repo"><button class="btn outline sm" data-act="repo-add" data-testid="dc-repo-add">${ic.plus} Add</button></span></div>
    <div class="lbar dcfor"><span class="muted sm">Install into</span>${hs.map((h) => `<label class="tchip"><input type="checkbox" data-act="dc-enable" data-h="${h.id}" ${on.includes(h.id) ? 'checked' : ''} aria-label="Install into ${esc(h.label)}" data-testid="dc-for-${h.id}">${esc(h.label.replace(/ Code$/, ''))}</label>`).join('')}<span class="muted sm">remembered for the next install; Installed ticks the rest</span></div>
    <div id="dc-list" data-testid="discover">${sk.disc ? '' : `<p class="empty">Reading ${repos.length} repositories… the first time downloads each one.</p>`}</div>`;
  // fetched after this draw, then drawn again; awaiting render() from inside a render would wait on itself
  if (!sk.disc && !sk.loading) {
    sk.loading = true;
    attempt(() => call('discover')).then((r) => { sk.disc = r || []; sk.loading = false; render(); });
    return;
  }
  if (!sk.disc) return;
  const list = document.getElementById('dc-list');
  list.innerHTML = (sk.disc || []).map((r) => `<section class="dgroup"><h5>${esc(r.repo)} <span>${r.error ? `<span class="err">${esc(r.error)}</span>` : `${r.skills.length} skills`}</span></h5>
    ${r.skills.map((s) => `<div class="drow" data-search="${esc((s.name + ' ' + (s.description || '') + ' ' + r.repo).toLowerCase())}" data-testid="dc-${esc(s.folder)}">
      <div><b>${esc(s.name)}</b>${s.path ? ` <span class="mono muted">${esc(s.path)}</span>` : ''}${s.description ? `<div class="md clamp" title="${esc(s.description)}">${esc(s.description)}</div>` : ''}</div>
      ${s.installed ? '<span class="tag">Installed</span>' : `<button class="btn outline sm" data-act="dc-install" data-url="${esc(s.url)}" data-folder="${esc(s.folder)}" data-testid="dc-install-${esc(s.folder)}">${ic.download} Install</button>`}</div>`).join('') || (r.error ? '' : '<p class="empty">No SKILL.md in this repository.</p>')}</section>`).join('');
  const q = document.getElementById('dc-q');
  q.oninput = () => { const t = q.value.trim().toLowerCase(); list.querySelectorAll('.drow').forEach((x) => { x.hidden = !!t && !x.dataset.search.includes(t); }); };
}

action('dc-enable', () => { localStorage.setItem(enableKey, JSON.stringify([...document.querySelectorAll('.dcfor input:checked')].map((i) => i.dataset.h))); });
action('skill-adopt', async (el) => {
  const names = el.dataset.name ? [el.dataset.name] : [];
  const r = await mutate(() => call('skill_adopt', { names }));
  if (!r) return;
  toast(`Now managed here: ${r.adopted.length === 1 ? r.adopted[0] : r.adopted.length + ' skills'}`);
  if (r.sync && r.sync.error) toast('Managed, but linking into the tools failed: ' + r.sync.error, true);
});
// a tool's header tick: every skill shown, on or off, in one write
action('skill-bulk', async (el) => {
  const items = [...el.closest('table').querySelectorAll('tbody tr[data-name]:not([hidden])')].map((tr) => tr.dataset.name);
  const on = el.checked;
  const rep = await mutate(() => call('target_bulk', { kind: 'skill', harness: el.dataset.h, items, on }), `${on ? 'Linked' : 'Unlinked'} ${items.length} ${items.length === 1 ? 'skill' : 'skills'} ${on ? 'into' : 'from'} ${el.dataset.h === 'claude' ? 'Claude Code' : el.dataset.h}`);
  syncReport(rep);
});
action('sk-tab', (el) => { sk.tab = el.dataset.v; render(); });
action('disc-refresh', () => {
  sk.disc = null; sk.loading = true; render();
  attempt(() => call('discover', { refresh: true })).then((r) => { sk.disc = r || []; sk.loading = false; render(); });
});
action('repo-add', async () => {
  const v = document.getElementById('dc-repo').value.trim();
  if (!v) return toast('Enter owner/repository or a GitHub link', true);
  if (await attempt(() => call('skill_repo_add', { repo: v }), 'Repository added')) { sk.disc = null; changed(); }
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
  ...(canUpdate({ source: el.dataset.source }) ? [{ label: 'Update from its source', run: async () => { if (await mutate(() => call('skill_update', { name: el.dataset.name }), `Updated ${el.dataset.name}`)) sk.updates[el.dataset.name] = false; } }] : []),
  { label: 'Open folder', run: () => attempt(() => call('open_folder', { path: el.dataset.path })) },
  { label: 'Remove…', danger: true, run: async () => {
    if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from every tool it is linked into?`, 'Remove')) {
      syncReport(await mutate(() => call('item_remove', { kind: 'skill', name: el.dataset.name })));
    }
  } },
]));

// Projects (1.10, Option A): the status, five readouts, and ONE list. Each project is one row: its name (with what
// tells it from another of the same name), its remote or path, its tags, its share of the chosen metric, cost, tokens,
// sessions, the range's sparkline on one shared scale, what runs now, and its actions. Fed by `projects`,
// `project_pin`, `project_alias`. The page's state is in the hash (#projects?days=30&sort=cost&metric=cost&per=25&page=2)
// through replaceState; every choice redraws through patch(), so the tiles stay the same nodes.

import { S, call, view, go, attempt, mutate, changed, openDialog, closeDialog, dlgHead, openMenu, action, drawPins, patch, render, editing } from './core.js';
import { esc, fmt, when, ic } from './lib.js';
import { computersStatus, syncOn, computers, nameOf, cdot } from './computers.js';
import { tagsHtml, href, money } from './sessions.js';
import { pageHead, seg, plural } from './parts.js';

const pj = { list: null, fetched: '', days: '7', sort: 'active', metric: 'tokens', per: 10, page: 0, tiles: {}, q: '' };
const pjHash = () => history.replaceState(null, '', href('projects', { days: pj.days === '7' ? '' : pj.days, sort: pj.sort === 'active' ? '' : pj.sort, metric: pj.metric === 'tokens' ? '' : pj.metric, per: pj.per === 10 ? '' : String(pj.per), page: pj.page ? String(pj.page) : '' }));
async function fetchProjects() {
  const [list, tl] = await Promise.all([call('projects', { days: pj.days === 'all' ? null : +pj.days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  pj.list = list; pj.fetched = pj.days; S.cache.tags = tl;
  drawPins(list);
}
/** What tells each project of `list` ({ key, name, root, remote }) from another of the same name: a Map from its key
 * to { hint, dup }. `hint` is its remote ("github.com/owner/repo"), or the last two folders of its path (more when two
 * same-named folders end alike); `dup` says another project has its name. A project with no folder on this computer
 * (it ran on another one) is told apart by its key, which is its path there. */
export function tellApart(list) {
  const tail = (s, n) => s.split(/[\\/]/).filter(Boolean).slice(-n).join(s.includes('\\') ? '\\' : '/');
  return new Map(list.map((p) => {
    const same = list.filter((o) => o.name.toLowerCase() === p.name.toLowerCase()), at = (o) => o.root || (same.length > 1 ? o.key : '') || '';
    let n = 2;
    while (!p.remote && n < 12 && same.some((o) => o !== p && !o.remote && at(o) !== at(p) && tail(at(o), n) === tail(at(p), n))) n++;
    return [p.key, { hint: p.remote || tail(at(p), n), dup: same.length > 1 }];
  }));
}
/** The area sparkline of a series; `max` puts every row on one y-scale (the critics' fix), alone it scales to itself. */
export const area = (s, max) => {
  const w = 300, h = 44, m = Math.max(1, max || 0, ...s), n = Math.max(2, s.length);
  const pts = s.map((v, i) => [i * (w / (n - 1)), h - 4 - (v / m) * (h - 8)]);
  const d = pts.map((q, i) => (i ? 'L' : 'M') + q[0].toFixed(1) + ' ' + q[1].toFixed(1)).join(' ');
  return `<svg class="a-area" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true"><path class="a-area-fill" d="${d} L${pts[pts.length - 1][0].toFixed(1)} ${h} L0 ${h}Z"/><path class="a-area-line" d="${d}"/><circle class="a-area-dot" cx="${pts[pts.length - 1][0].toFixed(1)}" cy="${pts[pts.length - 1][1].toFixed(1)}" r="2.5"/></svg>`;
};
export const pinBtn = (p) => `<button class="a-ib ${p.pinned ? 'on' : ''}" data-act="project-pin" data-key="${esc(p.key)}" data-on="${p.pinned ? '' : '1'}" title="${p.pinned ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar'}" aria-label="${p.pinned ? 'Unpin' : 'Pin'} ${esc(p.name)}" data-testid="pin-${esc(p.key)}">${ic.pin}</button>`;
const editBtn = (p) => `<button class="a-ib" data-act="project-rename" data-key="${esc(p.key)}" title="Rename (or double-click the name)" aria-label="Rename ${esc(p.name)}" data-testid="rename-${esc(p.key)}">${ic.edit}</button>`;
/** With sync on: a dot per computer that has the repository (the counts above sum them all), and whether this one has it. */
const pcDots = (p) => {
  const pcs = S.cache.computers;
  if (!syncOn(pcs) || !p.devices || !p.devices.length) return '';
  return `<span class="pc-dots" title="${esc(p.devices.map((d) => `${d.name || nameOf(pcs, d.id)}: ${plural(d.sessions, 'session')}`).join(', '))}" data-testid="pcard-pcs">${p.devices.map((d) => cdot(pcs, d.id, d.name, 'on')).join('')}${p.here === false ? '<span class="muted a-notcloned">not cloned here</span>' : ''}</span>`;
};
/** The name of a project, renamed in place by the pencil or a double-click. */
export const nameEl = (p) => `<span class="a-pname" data-name="${esc(p.key)}" title="Double-click to rename">${esc(p.name)}</span>`;

view('projects', async (sheet, params) => {
  pj.days = params.days || '7'; pj.sort = params.sort || 'active'; pj.metric = params.metric || 'tokens'; pj.per = +params.per || 10; pj.page = +params.page || 0;
  sheet.className = 'sheet';
  if (!pj.list || pj.fetched !== pj.days) {
    if (!sheet.querySelector('#pj-tiles')) sheet.innerHTML = `<div class="page-body">${pageHead('projects', 'Reading projects…')}</div>`;
    await fetchProjects();
  }
  // Ctrl K on this page narrows the rows to a name (the lazy user's fix)
  S.pageFind = (v) => { pj.q = (v || '').trim().toLowerCase(); draw(); };
  draw();
});

/** A segmented choice of the page's state: the range, the sort, the metric, the rows a page. */
const pseg = (k, opts, cur, label, tid) => seg(label, opts, cur, (v) => `data-act="p-set" data-k="${k}" data-v="${v}" data-testid="${tid}-${v}"`);

/** The whole page from pj; patch() keeps the tiles' nodes and flashes a number that changed since the last draw. */
function draw() {
  const sheet = document.getElementById('sheet');
  if (S.route.view !== 'projects' || !sheet) return;
  const list = pj.list, pcs = S.cache.computers, rangeTxt = pj.days === 'all' ? 'all time' : 'the last ' + esc(pj.days) + ' days';
  const liveN = list.filter((p) => p.live).length;
  const head = pageHead('projects', list.length ? `${plural(list.length, 'project')} ran in ${rangeTxt}${syncOn(pcs) ? ' on ' + plural(computers(pcs).length, 'computer') : ''}${liveN ? `; ${liveN} ${liveN === 1 ? 'has' : 'have'} a session open now` : ''}.` : `No project ran in ${rangeTxt}.`,
    `${pseg('days', [['7', '7 d'], ['30', '30 d'], ['all', 'All']], pj.days, 'Range', 'pdays')}<span class="label">Sort</span>${pseg('sort', [['active', 'Active'], ['cost', 'Cost'], ['tokens', 'Tokens'], ['name', 'Name']], pj.sort, 'Sort', 'psort')}`);
  if (!list.length) { patch(sheet, `<div class="page-body" data-tour="projects">${head}<section class="panel-card empty-state"><span class="muted">A project is a folder your tools ran in.</span><div>${pj.days !== 'all' ? '<button class="btn" data-act="p-set" data-k="days" data-v="all">Show all time</button>' : '<button class="btn primary" data-act="session-new">New session</button>'}</div></section></div>`); return; }
  // the five tiles; a number that changed since the last draw flashes once
  const tokens = list.reduce((a, p) => a + p.tokens, 0), sessions = list.reduce((a, p) => a + p.sessions, 0), live = list.reduce((a, p) => a + p.live, 0);
  const priced = list.filter((p) => p.cost != null), cost = priced.length ? priced.reduce((a, p) => a + p.cost, 0) : null;
  const nDays = pj.days === 'all' ? Math.max(1, ...list.map((p) => p.series.length)) : +pj.days;
  const busiest = [...list].sort((a, b) => b.tokens - a.tokens)[0];
  const t = { tokens: fmt(tokens), sessions: plural(sessions, 'session'), cost: money(cost), day: cost != null ? money(cost / nDays) + ' a day ≈' : 'no priced model yet', n: String(list.length), accounts: plural([...new Set(list.flatMap((p) => p.accounts))].length, 'account'), live: String(live), livenote: live ? list.filter((p) => p.live).map((p) => p.name).join(', ') : 'nothing running', busy: busiest ? busiest.name : '—', busynote: busiest && tokens ? Math.round((busiest.tokens / tokens) * 100) + '% of tokens' + (busiest.cost != null && cost ? ' · ' + Math.round((busiest.cost / cost) * 100) + '% of cost' : '') : '' };
  const was = pj.tiles, flash = (k) => (was[k] !== undefined && was[k] !== t[k] ? 'a-flash' : '');
  pj.tiles = t;
  const tile = (title, value, note, cls = '', extra = '') => `<div class="account-stat ${extra}" ${extra ? 'data-act="go-live" role="link" tabindex="0" title="Open the live sessions"' : ''}><span>${title}</span><b class="${cls}">${value}</b><span title="${esc(note)}">${esc(note)}</span></div>`;
  const tiles = `<section class="account-stats a-tiles" id="pj-tiles" data-testid="project-tiles" aria-label="Project readouts" data-tour="tiles">${tile('Tokens · ' + (pj.days === 'all' ? 'all time' : esc(pj.days) + ' days'), t.tokens, t.sessions, flash('tokens'))}${tile('At list prices', t.cost, t.day, flash('cost'))}${tile('Projects', t.n, t.accounts, flash('n'))}${tile('Sessions running now', t.live, t.livenote, (live ? 'a-live ' : '') + flash('live'), live ? 'a-tile-btn' : '')}${tile('Busiest', esc(t.busy), t.busynote, flash('busy'))}</section>`;
  // the rows of the current page, sorted; each with its share of the chosen metric, and every sparkline on the page's
  // one y-scale. The projects on no row of this page are summed in the "others" line, so the shares add up to all.
  const metric = pj.metric, val = (p) => (metric === 'cost' ? p.cost || 0 : metric === 'sessions' ? p.sessions : p.tokens), total = list.reduce((a, p) => a + val(p), 0) || 1, on = (k) => (metric === k ? 'on' : '');
  const found = pj.q ? list.filter((p) => `${p.name} ${p.remote || ''} ${p.root || ''}`.toLowerCase().includes(pj.q)) : list;
  const sorted = [...found].sort((a, b) => pj.sort === 'cost' ? (b.cost || 0) - (a.cost || 0) : pj.sort === 'tokens' ? b.tokens - a.tokens : pj.sort === 'name' ? a.name.localeCompare(b.name) : b.last_active_at - a.last_active_at);
  const pages = Math.max(1, Math.ceil(sorted.length / pj.per));
  if (pj.page >= pages) pj.page = pages - 1;
  const shown = sorted.slice(pj.page * pj.per, pj.page * pj.per + pj.per), others = list.filter((p) => !shown.includes(p)), peak = Math.max(1, ...shown.flatMap((p) => p.series));
  const period = pj.days === 'all' ? 'Activity' : pj.days + ' days', apart = tellApart(list);
  const share = (v) => `<span class="a-share"><svg class="a-track" aria-hidden="true"><rect width="${((v / total) * 100).toFixed(1)}%" height="100%" rx="3"/></svg><span class="num">${Math.round((v / total) * 100)}%</span></span>`;
  const row = (p) => { const a = apart.get(p.key), where = p.remote || p.root || ''; return `<div class="a-prow a-pcard ${p.live ? 'running' : ''}" data-act="project-open" data-key="${esc(p.key)}" role="link" tabindex="0" aria-label="Sessions of ${esc(p.name)}" data-testid="pcard-${esc(p.key)}">
      <div class="a-pwho"><div class="a-phd">${nameEl(p)}${a.dup && a.hint !== where ? `<span class="a-dup" data-testid="dup-hint">${esc(a.hint)}</span>` : ''}${tagsHtml(p.tags, 'project', p.key, p.name)}${editBtn(p)}</div><span class="a-path mono" title="${esc(where)}">${esc(where)}</span></div>
      ${share(val(p))}
      <span class="num ${on('cost')}">${money(p.cost)}${p.cost != null && p.priced_share < 0.95 ? `<small>${Math.round(p.priced_share * 100)}% priced</small>` : ''}</span><span class="num ${on('tokens')}">${fmt(p.tokens)}</span><span class="num ${on('sessions')}">${p.sessions}<small>${plural(p.accounts.length, 'account')}</small></span>
      <div class="a-spark" title="Tokens a day, ${esc(period.toLowerCase())}; every row on one scale, peak ${fmt(peak)}">${area(p.series, peak)}</div>
      <div class="a-pmeta"><span class="${p.live ? 'a-live' : ''}">${p.live ? p.live + ' live' : esc(when(p.last_active_at))}</span>${pcDots(p)}</div>
      <div class="a-pacts">${pinBtn(p)}${p.root ? `<button class="a-ib" data-act="project-new" data-dir="${esc(p.root)}" data-name="${esc(p.name)}" data-profile="" title="New session here" aria-label="New session in ${esc(p.name)}">${ic.play}</button>` : '<span class="a-ib"></span>'}<button class="a-ib" data-act="project-menu" data-key="${esc(p.key)}" data-name="${esc(p.name)}" data-root="${esc(p.root)}" data-pinned="${p.pinned ? 1 : ''}" title="More" aria-label="More for ${esc(p.name)}" aria-haspopup="menu">${ic.dots}</button></div></div>`; };
  const sum = (f) => others.reduce((a, p) => a + f(p), 0);
  const rest = others.length ? `<div class="a-prow other" data-testid="project-others"><span class="muted">${plural(others.length, 'other project')}</span>${share(sum(val))}<span class="num ${on('cost')}">${money(others.some((p) => p.cost != null) ? sum((p) => p.cost || 0) : null)}</span><span class="num ${on('tokens')}">${fmt(sum((p) => p.tokens))}</span><span class="num ${on('sessions')}">${sum((p) => p.sessions)}</span><span></span><span></span><span></span></div>` : '';
  const pager = sorted.length > 10 ? `<div class="a-pager" id="pj-pager"><button class="btn sm icon" data-act="p-set" data-k="page" data-v="${Math.max(0, pj.page - 1)}" ${pj.page === 0 ? 'disabled' : ''} aria-label="Previous page">‹</button>${Array.from({ length: pages }, (_, i) => `<button class="btn sm ${i === pj.page ? 'on' : 'quiet'}" data-act="p-set" data-k="page" data-v="${i}" aria-pressed="${i === pj.page}" data-testid="ppage-${i}">${i + 1}</button>`).join('')}<button class="btn sm icon" data-act="p-set" data-k="page" data-v="${Math.min(pages - 1, pj.page + 1)}" ${pj.page === pages - 1 ? 'disabled' : ''} aria-label="Next page">›</button>
      <span>${pj.page * pj.per + 1}–${Math.min(sorted.length, (pj.page + 1) * pj.per)} of ${sorted.length}</span>${pseg('per', [[10, '10'], [25, '25'], [50, '50']], pj.per, 'Rows a page', 'pper')}<span>a page</span></div>` : '';
  patch(sheet, `<div class="page-body" data-tour="projects">${head}${tiles}
    <section class="panel-card a-plist" data-testid="project-dist" aria-label="The projects, and where the tokens and the cost went"><div class="panel-head"><h2 class="panel-title">Where the tokens and the cost went</h2><span class="row"><span class="label">Share of</span>${pseg('metric', [['tokens', 'Tokens'], ['cost', 'Cost'], ['sessions', 'Sessions']], metric, 'Share of', 'pmetric')}</span></div>
      <div class="a-prow head"><span>Project</span><span>Share of ${esc(metric)}</span><span class="num ${on('cost')}">Cost ≈</span><span class="num ${on('tokens')}">Tokens</span><span class="num ${on('sessions')}">Sessions</span><span title="Tokens a day; every row on one scale">${esc(period)} · peak ${fmt(peak)}</span><span class="num">Last active</span><span></span></div>
      <div data-testid="project-cards" data-tour="cards">${shown.map(row).join('') || '<div class="a-empty">No project matches.</div>'}${rest}</div>${pager}</section></div>`);
}

/** Called by the background refresh: new numbers in place, nothing else moves. */
export async function refreshProjects() {
  if (S.route.view !== 'projects' || !pj.list) return;
  await fetchProjects().catch(() => {});
  if (editing()) { S.dirty = true; return; } // a name being typed (or a dialog) stays as it is; the page catches up when it closes
  draw();
}
// an unpin from the sidebar: the Projects page fetches again; a project's header icon flips
document.addEventListener('hm:unpinned', (e) => {
  if (S.route.view === 'projects') { refreshProjects(); return; }
  const b = document.querySelector(`[data-act=project-pin][data-key="${CSS.escape(e.detail)}"]`);
  if (b) { b.classList.remove('on'); b.dataset.on = '1'; b.title = 'Pin to the sidebar'; }
});

action('p-set', async (el) => {
  const k = el.dataset.k, v = el.dataset.v;
  if (k === 'days') { pj.days = v; pj.page = 0; pjHash(); await fetchProjects(); draw(); return; }
  if (k === 'per') { pj.per = +v; pj.page = 0; } else if (k === 'page') pj.page = +v; else pj[k] = v;
  pjHash(); draw();
});
let openTimer;
action('project-open', (el, e) => {
  if (e.target.closest('input')) return;
  clearTimeout(openTimer);
  // a click on the name may be the first half of a double-click (rename): it opens only when no second click comes
  if (e.target.closest('.a-pname[data-name]')) openTimer = setTimeout(() => go('sessions', { project: el.dataset.key }), 280);
  else go('sessions', { project: el.dataset.key });
});
// the pin: the icon, the row's menu and the sidebar change; the page does not redraw
action('project-pin', async (el) => {
  const on = el.dataset.on === '1';
  if (!(await attempt(() => call('project_pin', { key: el.dataset.key, on }), on ? 'Pinned to the sidebar' : 'Unpinned'))) return;
  const p = (pj.list || []).find((x) => x.key === el.dataset.key);
  if (p) p.pinned = on;
  if (S.route.view === 'projects') { draw(); drawPins(pj.list); return; }
  // elsewhere (a project's Sessions header): the icon flips and the sidebar follows, nothing else moves
  el.classList.toggle('on', on); el.dataset.on = on ? '' : '1'; el.title = on ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar';
  call('projects', { days: 30 }).then(drawPins).catch(() => {});
});
/** Rename a project where its name is: the pencil, a double-click, or the menu. Enter saves, Esc cancels. */
function renameProject(key) {
  const el = document.querySelector(`.a-pname[data-name="${CSS.escape(key)}"]`);
  if (!el || el.querySelector('input')) return;
  const old = el.textContent;
  el.innerHTML = `<input class="a-rename" value="${esc(old)}" aria-label="Project name" maxlength="80" data-testid="rename-input">`;
  const inp = el.querySelector('input');
  inp.focus(); inp.select();
  let done = false;
  const finish = async (save) => {
    if (done) return; done = true;
    const name = inp.value.trim();
    el.textContent = save && name ? name : old;
    if (save && name && name !== old) {
      if (await attempt(() => call('project_alias', { key, name }), `Renamed to ${name}`)) { for (const p of pj.list || []) if (p.key === key) p.name = name; if (pj.list) drawPins(pj.list); }
      else el.textContent = old;
    }
    // a background refresh that arrived while the name was being typed catches up now
    if (S.dirty && !editing()) { S.dirty = false; if (S.route.view === 'projects') refreshProjects(); else render(true); }
  };
  inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { e.preventDefault(); finish(true); } if (e.key === 'Escape') { e.preventDefault(); finish(false); } });
  inp.addEventListener('blur', () => finish(true));
  inp.addEventListener('click', (e) => e.stopPropagation());
}
action('project-rename', (el) => renameProject(el.dataset.key));
document.addEventListener('dblclick', (e) => { const n = e.target.closest('.a-pname[data-name]'); if (n && !n.querySelector('input')) { clearTimeout(openTimer); e.preventDefault(); renameProject(n.dataset.name); } });
action('project-menu', (el) => openMenu(el, [
  ...(S.route.view !== 'sessions' || S.route.params.project !== el.dataset.key ? [{ label: 'Open its sessions', run: () => go('sessions', { project: el.dataset.key }) }] : []),
  { label: el.dataset.pinned ? 'Unpin from the sidebar' : 'Pin to the sidebar', run: async () => { if (await attempt(() => call('project_pin', { key: el.dataset.key, on: !el.dataset.pinned }), el.dataset.pinned ? 'Unpinned' : 'Pinned to the sidebar')) { if (S.route.view === 'projects') refreshProjects(); else changed(); } } },
  { label: 'Rename project', run: () => (document.querySelector(`.a-pname[data-name="${CSS.escape(el.dataset.key)}"]`) ? renameProject(el.dataset.key) : projectDialog(el.dataset.key, el.dataset.name, 'rename')) },
  { label: 'Merge into another project', run: () => projectDialog(el.dataset.key, el.dataset.name, 'merge') },
  { label: 'Reset name', danger: true, run: () => mutate(() => call('project_alias', { key: el.dataset.key }), 'Reset') },
]));
function projectDialog(key, name, mode) {
  openDialog(`${dlgHead(mode === 'rename' ? 'Rename project' : 'Merge project')}
    <dl class="facts-grid"><dt>Project</dt><dd>${esc(name)} <span class="mono muted">${esc(key)}</span></dd></dl>
    <div class="form-field"><label for="pj-v">${mode === 'rename' ? 'New name' : 'Merge into key'}</label><input id="pj-v" value="${mode === 'rename' ? esc(name) : ''}">${mode === 'merge' ? '<span class="a-hint">The key of the project to merge into, as shown under its name.</span>' : ''}</div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="pj-ok">Save</button></span></div>`, { size: 'sm', label: mode === 'rename' ? 'Rename project' : 'Merge project' });
  document.getElementById('pj-ok').onclick = async () => {
    const v = document.getElementById('pj-v').value.trim();
    if (await attempt(() => call('project_alias', mode === 'rename' ? { key, name: v } : { key, merge_into: v }), 'Saved')) { closeDialog(); changed(); }
  };
}

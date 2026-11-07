// Projects as the reference draws them (round2/app/pages-a.js): the finding, five readouts, where the tokens and the
// cost went, and one card per project with a sparkline on one shared scale. Fed by `projects`, `project_pin`,
// `project_alias`. The page's state is in the hash (#projects?days=30&sort=cost&metric=cost&per=25&page=2) through
// replaceState; every choice redraws through patch(), so the tiles stay the same nodes.

import { S, call, view, go, attempt, mutate, changed, openDialog, closeDialog, dlgHead, openMenu, action, drawPins, patch } from './core.js';
import { esc, fmt, when, ic } from './lib.js';
import { computersStatus, syncOn, computers, nameOf, cdot } from './computers.js';
import { tagsHtml, href, money } from './sessions.js';

const SH = ['a-l1', 'a-l2', 'a-l3', 'a-l4', 'a-l5'];
const pj = { list: null, fetched: '', days: '7', sort: 'active', metric: 'tokens', per: 10, page: 0, tiles: {}, q: '', distShut: false };
const plural = (n, one, many) => `${n} ${n === 1 ? one : many || one + 's'}`;
const pjHash = () => history.replaceState(null, '', href('projects', { days: pj.days === '7' ? '' : pj.days, sort: pj.sort === 'active' ? '' : pj.sort, metric: pj.metric === 'tokens' ? '' : pj.metric, per: pj.per === 10 ? '' : String(pj.per), page: pj.page ? String(pj.page) : '' }));
async function fetchProjects() {
  const [list, tl] = await Promise.all([call('projects', { days: pj.days === 'all' ? null : +pj.days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  pj.list = list; pj.fetched = pj.days; S.cache.tags = tl;
  drawPins(list);
}
/** The area sparkline of a series; `max` puts every card on one y-scale (the critics' fix), alone it scales to itself. */
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
    if (!sheet.querySelector('#pj-tiles')) sheet.innerHTML = '<div class="page-body"><h1 class="muted">Reading projects…</h1></div>';
    await fetchProjects();
  }
  // Ctrl K on this page narrows the cards to a name (the lazy user's fix)
  S.pageFind = (v) => { pj.q = (v || '').trim().toLowerCase(); draw(); };
  draw();
});

const chips = (k, opts, cur, label, tid) => `<div class="chip-group" role="group" aria-label="${label}">${opts.map(([v, l]) => `<button class="filter-chip ${cur === v ? 'active' : ''}" data-act="p-set" data-k="${k}" data-v="${v}" aria-pressed="${cur === v}" data-testid="${tid}-${v}">${l}</button>`).join('')}</div>`;

/** The whole page from pj; patch() keeps the tiles' nodes and flashes a number that changed since the last draw. */
function draw() {
  const sheet = document.getElementById('sheet');
  if (S.route.view !== 'projects' || !sheet) return;
  const list = pj.list, pcs = S.cache.computers, rangeTxt = pj.days === 'all' ? 'all time' : 'the last ' + pj.days + ' days';
  const liveN = list.filter((p) => p.live).length;
  const head = `<header class="page-heading"><div><h1>${list.length ? `${plural(list.length, 'project')} ran in ${rangeTxt}${liveN ? `; ${liveN} ${liveN === 1 ? 'has' : 'have'} a session open now` : ''}.` : 'No sessions in this range yet.'}</h1><div class="heading-facts">${pj.days === 'all' ? 'All time' : pj.days + ' days'}${syncOn(pcs) ? ' · ' + plural(computers(pcs).length, 'computer') : ''} · cost ≈ at list prices</div></div>
    <div class="page-actions">${chips('days', [['7', '7d'], ['30', '30d'], ['all', 'All']], pj.days, 'Range', 'pdays')}${chips('sort', [['active', 'Active'], ['cost', 'Cost'], ['tokens', 'Tokens'], ['name', 'Name']], pj.sort, 'Sort', 'psort')}</div></header>`;
  if (!list.length) { patch(sheet, `<div class="page-body" data-tour="projects">${head}<section class="panel-card empty-state"><span class="muted">A project is a folder your tools ran in.</span><div>${pj.days !== 'all' ? '<button class="btn-secondary" data-act="p-set" data-k="days" data-v="all">Show all time</button>' : '<button class="btn-primary" data-act="session-new">New session</button>'}</div></section></div>`); return; }
  // the five tiles; a number that changed since the last draw flashes once
  const tokens = list.reduce((a, p) => a + p.tokens, 0), sessions = list.reduce((a, p) => a + p.sessions, 0), live = list.reduce((a, p) => a + p.live, 0);
  const priced = list.filter((p) => p.cost != null), cost = priced.length ? priced.reduce((a, p) => a + p.cost, 0) : null;
  const nDays = pj.days === 'all' ? Math.max(1, ...list.map((p) => p.series.length)) : +pj.days;
  const busiest = [...list].sort((a, b) => b.tokens - a.tokens)[0];
  const t = { tokens: fmt(tokens), sessions: plural(sessions, 'session'), cost: money(cost), day: cost != null ? money(cost / nDays) + ' a day ≈' : 'no priced model yet', n: String(list.length), accounts: plural([...new Set(list.flatMap((p) => p.accounts))].length, 'account'), live: String(live), livenote: live ? list.filter((p) => p.live).map((p) => p.name).join(', ') : 'nothing running', busy: busiest ? busiest.name : '—', busynote: busiest && tokens ? Math.round((busiest.tokens / tokens) * 100) + '% of tokens' + (busiest.cost != null && cost ? ' · ' + Math.round((busiest.cost / cost) * 100) + '% of cost' : '') : '' };
  const was = pj.tiles, flash = (k) => (was[k] !== undefined && was[k] !== t[k] ? 'a-flash' : '');
  pj.tiles = t;
  const tile = (title, value, note, cls = '', extra = '') => `<div class="account-stat ${extra}" ${extra ? 'data-act="go-live" role="link" tabindex="0" title="Open the live sessions"' : ''}><span>${title}</span><b class="${cls}">${value}</b><span title="${esc(note)}">${esc(note)}</span></div>`;
  const tiles = `<section class="account-stats a-tiles" id="pj-tiles" data-testid="project-tiles" aria-label="Project readouts" data-tour="tiles">${tile('Tokens · ' + (pj.days === 'all' ? 'all time' : pj.days + ' days'), t.tokens, t.sessions, flash('tokens'))}${tile('At list prices', t.cost, t.day, flash('cost'))}${tile('Projects', t.n, t.accounts, flash('n'))}${tile('Sessions running now', t.live, t.livenote, (live ? 'a-live ' : '') + flash('live'), live ? 'a-tile-btn' : '')}${tile('Busiest', esc(t.busy), t.busynote, flash('busy'))}</section>`;
  // where the tokens and the cost went: the top five and the rest, each with a share track of the chosen metric
  const metric = pj.metric, val = (p) => (metric === 'cost' ? p.cost || 0 : metric === 'sessions' ? p.sessions : p.tokens), total = list.reduce((a, p) => a + val(p), 0) || 1;
  const top = [...list].sort((a, b) => val(b) - val(a)), head5 = top.slice(0, 5), others = top.slice(5), oVal = others.reduce((a, p) => a + val(p), 0), on = (k) => (metric === k ? 'on' : '');
  const prow = (name, key, v, c, tok, n, sw) => `<div class="a-prow"><span class="a-sw ${sw}"></span>${key ? `<a class="a-nm" href="${href('sessions', { project: key })}">${esc(name)}</a>` : `<span class="a-nm muted">${esc(name)}</span>`}<svg class="a-track" aria-hidden="true"><rect class="${sw}" width="${((v / total) * 100).toFixed(1)}%" height="100%" rx="3"/></svg><span class="num ${on('cost')}">${money(c)}</span><span class="num ${on('tokens')}">${fmt(tok)}</span><span class="num ${on('sessions')}">${n}</span><span class="num">${Math.round((v / total) * 100)}%</span></div>`;
  const dist = `<section class="panel-card ${pj.distShut ? 'a-shut' : ''}" data-testid="project-dist" aria-label="Where the tokens and the cost went"><div class="panel-head"><button class="panel-title fold" data-act="p-dist" aria-expanded="${!pj.distShut}">Where the tokens and the cost went</button>${chips('metric', [['tokens', 'Tokens'], ['cost', 'Cost'], ['sessions', 'Sessions']], metric, 'Metric', 'pmetric')}</div>
    <div class="a-prow head"><span></span><span>Project</span><span>Share of ${metric}</span><span class="num ${on('cost')}">Cost ≈</span><span class="num ${on('tokens')}">Tokens</span><span class="num ${on('sessions')}">Sessions</span><span class="num">Share</span></div>
    ${head5.map((p, i) => prow(p.name, p.key, val(p), p.cost, p.tokens, p.sessions, SH[i])).join('')}${others.length ? prow(`${others.length} other ${others.length === 1 ? 'project' : 'projects'}`, '', oVal, others.some((p) => p.cost != null) ? others.reduce((a, p) => a + (p.cost || 0), 0) : null, others.reduce((a, p) => a + p.tokens, 0), others.reduce((a, p) => a + p.sessions, 0), 'a-l0') : ''}</section>`;
  // the cards of the current page, sorted; every sparkline on the page's one y-scale, with its period
  const found = pj.q ? list.filter((p) => `${p.name} ${p.remote || ''} ${p.root || ''}`.toLowerCase().includes(pj.q)) : list;
  const sorted = [...found].sort((a, b) => pj.sort === 'cost' ? (b.cost || 0) - (a.cost || 0) : pj.sort === 'tokens' ? b.tokens - a.tokens : pj.sort === 'name' ? a.name.localeCompare(b.name) : b.last_active_at - a.last_active_at);
  const pages = Math.max(1, Math.ceil(sorted.length / pj.per));
  if (pj.page >= pages) pj.page = pages - 1;
  const shown = sorted.slice(pj.page * pj.per, pj.page * pj.per + pj.per), peak = Math.max(1, ...shown.flatMap((p) => p.series));
  const period = pj.days === 'all' ? 'activity' : pj.days + ' days';
  const card = (p) => `<div class="panel-card a-pcard ${p.live ? 'running' : ''}" data-act="project-open" data-key="${esc(p.key)}" role="link" tabindex="0" aria-label="Sessions of ${esc(p.name)}" data-testid="pcard-${esc(p.key)}">
      <div class="a-pwho"><div class="a-phd">${nameEl(p)}${editBtn(p)}</div><span class="a-path mono" title="${esc(p.remote || p.root)}">${esc(p.remote || p.root)}</span><div>${tagsHtml(p.tags, 'project', p.key, p.name)}</div></div>
      <div class="a-spark" title="Tokens a day, ${esc(period)}; every card on one scale, peak ${fmt(peak)}">${area(p.series, peak)}<small>${esc(period)} · peak ${fmt(peak)}</small></div>
      <div class="a-pnum"><b>${money(p.cost)}</b><small>${fmt(p.tokens)} tokens${p.cost != null && p.priced_share < 0.95 ? ' · ' + Math.round(p.priced_share * 100) + '% priced' : ''}</small></div>
      <div class="a-pmeta"><span>${plural(p.sessions, 'session')} · ${plural(p.accounts.length, 'account')}</span><span class="${p.live ? 'a-live' : ''}">${p.live ? p.live + ' live' : esc(when(p.last_active_at))}</span>${pcDots(p)}</div>
      <div class="a-pacts">${pinBtn(p)}${p.root ? `<button class="a-ib" data-act="project-new" data-dir="${esc(p.root)}" data-name="${esc(p.name)}" data-profile="" title="New session here" aria-label="New session in ${esc(p.name)}">${ic.play}</button>` : ''}<button class="a-ib" data-act="project-menu" data-key="${esc(p.key)}" data-name="${esc(p.name)}" data-root="${esc(p.root)}" data-pinned="${p.pinned ? 1 : ''}" title="More" aria-label="More for ${esc(p.name)}" aria-haspopup="menu">${ic.dots}</button></div></div>`;
  const cards = `<div class="a-pcards" data-testid="project-cards" data-tour="cards">${shown.map(card).join('') || '<section class="a-grp"><div class="a-empty">No project matches.</div></section>'}</div>`;
  const pager = sorted.length > 10 ? `<div class="a-pager" id="pj-pager"><button class="filter-chip" data-act="p-set" data-k="page" data-v="${Math.max(0, pj.page - 1)}" ${pj.page === 0 ? 'disabled' : ''} aria-label="Previous page">‹</button>${Array.from({ length: pages }, (_, i) => `<button class="filter-chip ${i === pj.page ? 'active' : ''}" data-act="p-set" data-k="page" data-v="${i}" data-testid="ppage-${i}">${i + 1}</button>`).join('')}<button class="filter-chip" data-act="p-set" data-k="page" data-v="${Math.min(pages - 1, pj.page + 1)}" ${pj.page === pages - 1 ? 'disabled' : ''} aria-label="Next page">›</button>
      <span>· ${pj.page * pj.per + 1}–${Math.min(sorted.length, (pj.page + 1) * pj.per)} of ${sorted.length} ·</span>${[10, 25, 50].map((n) => `<button class="filter-chip ${pj.per === n ? 'active' : ''}" data-act="p-set" data-k="per" data-v="${n}" data-testid="pper-${n}">${n}</button>`).join('')}<span>a page</span></div>` : '';
  patch(sheet, `<div class="page-body" data-tour="projects">${head}${tiles}${dist}${cards}${pager}</div>`);
}

/** Called by the background refresh: new numbers in place, nothing else moves. */
export async function refreshProjects() {
  if (S.route.view !== 'projects' || !pj.list) return;
  await fetchProjects().catch(() => {});
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
action('p-dist', () => { pj.distShut = !pj.distShut; draw(); });
let openTimer;
action('project-open', (el, e) => {
  if (e.target.closest('input')) return;
  clearTimeout(openTimer);
  // a click on the name may be the first half of a double-click (rename): it opens only when no second click comes
  if (e.target.closest('[data-name]')) openTimer = setTimeout(() => go('sessions', { project: el.dataset.key }), 280);
  else go('sessions', { project: el.dataset.key });
});
// the pin: the icon, the row's menu and the sidebar change; the page does not redraw
action('project-pin', async (el) => {
  const on = el.dataset.on === '1';
  if (!(await attempt(() => call('project_pin', { key: el.dataset.key, on }), on ? 'Pinned to the sidebar' : 'Unpinned'))) return;
  const p = (pj.list || []).find((x) => x.key === el.dataset.key);
  if (p) p.pinned = on;
  if (S.route.view === 'projects') { draw(); drawPins(pj.list); return; }
  // elsewhere (a project's Sessions header, the rail): the icon flips and the sidebar follows, nothing else moves
  el.classList.toggle('on', on); el.dataset.on = on ? '' : '1'; el.title = on ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar';
  call('projects', { days: 30 }).then(drawPins).catch(() => {});
});
/** Rename a project where its name is: the pencil, a double-click, or the menu. Enter saves, Esc cancels. */
export function renameProject(key) {
  const el = document.querySelector(`[data-name="${CSS.escape(key)}"]`);
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
    if (!save || !name || name === old) return;
    if (!(await attempt(() => call('project_alias', { key, name }), `Renamed to ${name}`))) { el.textContent = old; return; }
    for (const p of pj.list || []) if (p.key === key) p.name = name;
    if (pj.list) drawPins(pj.list);
  };
  inp.addEventListener('keydown', (e) => { e.stopPropagation(); if (e.key === 'Enter') { e.preventDefault(); finish(true); } if (e.key === 'Escape') { e.preventDefault(); finish(false); } });
  inp.addEventListener('blur', () => finish(true));
  inp.addEventListener('click', (e) => e.stopPropagation());
}
action('project-rename', (el) => renameProject(el.dataset.key));
document.addEventListener('dblclick', (e) => { const n = e.target.closest('[data-name]'); if (n && !n.querySelector('input')) { clearTimeout(openTimer); e.preventDefault(); renameProject(n.dataset.name); } });
action('project-menu', (el) => openMenu(el, [
  ...(S.route.view !== 'sessions' || S.route.params.project !== el.dataset.key ? [{ label: 'Open its sessions', run: () => go('sessions', { project: el.dataset.key }) }] : []),
  { label: el.dataset.pinned ? 'Unpin from the sidebar' : 'Pin to the sidebar', run: async () => { if (await attempt(() => call('project_pin', { key: el.dataset.key, on: !el.dataset.pinned }), el.dataset.pinned ? 'Unpinned' : 'Pinned to the sidebar')) { if (S.route.view === 'projects') refreshProjects(); else changed(); } } },
  { label: 'Rename project', run: () => (document.querySelector(`[data-name="${CSS.escape(el.dataset.key)}"]`) ? renameProject(el.dataset.key) : projectDialog(el.dataset.key, el.dataset.name, 'rename')) },
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

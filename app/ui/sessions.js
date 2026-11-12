// Sessions (the list) and Session (the reading view) as the reference draws them (round2/app/pages-a.js), fed by the
// index: `sessions`, `projects`, `tag_list`, `computers_status`, `transcript`, `session_stats`, `file_stamp`.
// The tags shared with Projects and Accounts live here too. Every redraw goes through patch(), so a live arrival
// keeps the reader's scroll, focus and open folds.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, openDialog, closeDialog, dlgHead, openMenu, action, render, saveFile, patch, drawPins, loadState } from './core.js';
import { esc, fmt, hm, when, toolIcon, ic, md } from './lib.js';
import { computersStatus, syncOn, computers, nameOf, cdot } from './computers.js';
import { resumeData, reasonShort, canResume } from './resume.js';
import { nameEl, pinBtn, area, refreshProjects } from './projects.js';
import './restore.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };
const plural = (n, one, many) => `${n} ${n === 1 ? one : many || one + 's'}`;

// ---------- tags ----------
// a tag's colour follows its place in the list (oldest first), so it never changes; the six hues are none of the state colours
const tagColor = (t) => 'c' + (Math.max(0, (S.cache.tags || []).findIndex((x) => x.tag === t)) % 6);
/** Tag chips for a session or project: the tag button with the count, up to two chips (each a link to that tag's list), +N. */
export function tagsHtml(tags, kind, key, name, editable = true) {
  const all = tags || [], shown = all.slice(0, 2), more = all.slice(2);
  const chip = (t) => `<a class="a-tag ${tagColor(t)}" href="${href('sessions', { tag: t })}" data-act="tag-go" data-tag="${esc(t)}" title="Sessions tagged ${esc(t)}">${esc(t)}</a>`;
  return `<span class="a-tags">${editable ? `<button class="a-tagbtn" data-act="tag-edit" data-kind="${kind}" data-key="${esc(key)}" data-name="${esc(name)}" data-tags="${esc(JSON.stringify(all))}" title="${all.length ? esc(all.join(', ')) : 'Add a tag'}" aria-label="Tags of ${esc(name)}">${ic.tag}${all.length ? `<small>${all.length}</small>` : ''}</button>` : ''}${shown.map(chip).join('')}${more.length ? `<span class="a-tag" title="${esc(more.join(', '))}">+${more.length}</span>` : ''}</span>`;
}
action('tag-go', (el) => go('sessions', { tag: el.dataset.tag }));
action('tag-edit', async (el) => {
  const { kind, key, name } = el.dataset;
  const mine = new Set(JSON.parse(el.dataset.tags || '[]'));
  const known = await attempt(() => call('tag_list')) || [];
  S.cache.tags = known;
  for (const t of mine) if (!known.some((k) => k.tag === t)) known.push({ tag: t, sessions: 0, projects: 0 });
  const row = (t) => `<label class="a-tagrow a-check"><input type="checkbox" value="${esc(t.tag)}" ${mine.has(t.tag) ? 'checked' : ''}><span class="a-tag ${tagColor(t.tag)}">${esc(t.tag)}</span><span class="a-cnt">${t.sessions + t.projects || ''}</span></label>`;
  openDialog(`${dlgHead('Tags of ' + name)}
    <div class="form-field"><label for="tg-new">New tag</label><input id="tg-new" placeholder="Type a tag and press Enter to add it" data-testid="tag-new"><span class="a-hint">Tags are yours: free text, shared across tools and accounts, never written into the tools' files.${kind === 'project' ? ' A project\'s tag reaches every session of it in filters.' : ''}</span></div>
    <div class="a-tagrows" id="tg-list" data-testid="tag-list">${known.map(row).join('') || '<span class="muted">No tags yet.</span>'}</div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="tg-ok" data-testid="tag-save">Save</button></span></div>`, { label: 'Tags of ' + name });
  const input = document.getElementById('tg-new'), list = document.getElementById('tg-list');
  input.onkeydown = (e) => {
    if (e.key !== 'Enter') return;
    e.preventDefault();
    const t = input.value.trim().replace(/\s+/g, '-').toLowerCase();
    if (!t) return;
    const box = [...list.querySelectorAll('input')].find((i) => i.value === t);
    if (box) box.checked = true; else { if (!known.length) list.innerHTML = ''; known.push({ tag: t, sessions: 0, projects: 0 }); list.insertAdjacentHTML('beforeend', row({ tag: t, sessions: 0, projects: 0 })); list.lastElementChild.querySelector('input').checked = true; }
    input.value = '';
  };
  document.getElementById('tg-ok').onclick = async () => {
    if (input.value.trim()) input.onkeydown({ key: 'Enter', preventDefault() {} });
    const tags = [...list.querySelectorAll('input:checked')].map((i) => i.value);
    closeDialog();
    if (!(await attempt(() => call('tag_set', { kind, key, tags }), tags.length ? `Tagged ${tags.join(', ')}` : 'Tags cleared'))) return;
    if (S.route.view === 'projects') refreshProjects(); else changed();
  };
});

// ---------- sessions ----------
// the list's state lives in the hash (#sessions?group=date&days=30&live=1&project=<key>&q=...), so every filter is a link
export const clean = (p) => Object.fromEntries(Object.entries(p).filter(([, v]) => v !== '' && v != null));
const sq = (patchParams) => go('sessions', clean({ ...S.route.params, ...patchParams }));
export const href = (view, p) => '#' + view + (Object.keys(clean(p)).length ? '?' + new URLSearchParams(clean(p)) : '');
export const money = (c, d = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d }));
const sMore = new Set(), sFold = new Set();
let railMore = false;
// the page reads the index in pages of 50 rows; the search, the computer and every filter are the backend's
const PER = 50;
const toolPill = (h) => `<span class="a-toolpill" title="${esc(TOOL[h] || h)}">${toolIcon(h, 13)}<span>${esc(TOOL[h] || h)}</span></span>`;
const seg = (k, opts, cur, label, tid) => `${label ? `<span class="a-lbl">${label}</span>` : ''}<div class="chip-group" role="group" aria-label="${label || k}">${opts.map(([v, l]) => `<button class="filter-chip ${cur === v ? 'active' : ''}" data-act="s-set" data-k="${k}" data-v="${esc(v)}" aria-pressed="${cur === v}" data-testid="${tid(v)}">${esc(l)}</button>`).join('')}</div>`;

/** One row: status dot, title and where, tags, account, tool, computer (sync on), tokens, when, one action. */
function sessionRow(s, pcs) {
  const sync = syncOn(pcs), remote = !!(pcs && pcs.this && s.device && s.device !== pcs.this.id);
  const device = s.device_name || nameOf(pcs, s.device), far = s.open_elsewhere ? device : '';
  const path = s.cwd ? s.cwd.split(/[\\/]/).slice(-2).join('\\') : '';
  const where = `${s.continued_from ? `<span data-testid="continued-${esc(s.id)}">continued from ${esc(s.continued_from)}</span> · ` : ''}${plural(s.messages, 'msg')}${path ? ` · <span class="mono">${esc(path)}</span>` : ''}`;
  // archived: read only; open: follow; from another computer: resume here when its repository and account are here, else why not
  const act = s.archived ? '' : s.open ? `<button class="btn-follow" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" ${remote ? `data-device="${esc(s.device)}"` : ''}>Follow</button>`
    : remote ? (canResume(s.resumable) ? `<button class="btn-lane-start" data-act="resume-here" data-s="${resumeData(s)}" data-testid="resume-here-${esc(s.id)}">Resume here</button>` : `<span class="a-reason" title="${esc(s.resumable || '')}" data-testid="reason-${esc(s.id)}">${esc(reasonShort(s.resumable) || 'on ' + device)}</span>`)
    : `<button class="btn-lane-start" data-act="resume" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-cwd="${esc(s.cwd)}">Resume</button>`;
  return `<div class="a-srow" data-key="row:${esc(s.profile)}/${esc(s.id)}" data-testid="session-${esc(s.id)}"><i class="${s.open ? 'live-pulse-dot' : 'a-dot' + (s.failed ? ' bad' : '')}" ${s.failed ? `title="${plural(s.failed, 'failed command')}"` : 'aria-hidden="true"'}></i>
    <div class="a-tcell"><button class="a-title" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" ${remote && !s.archived ? `data-device="${esc(s.device)}"` : ''} data-testid="read-${esc(s.id)}" title="${esc(s.cwd || 'No folder')}"><b>${esc(s.title || 'Untitled session')}${s.archived ? `<span class="a-pill arch" data-testid="archived-${esc(s.id)}">archived</span>` : ''}</b></button><div class="a-tline"><span class="a-where" title="${esc(s.cwd)}">${where}</span>${tagsHtml(s.tags, 'session', s.profile + '/' + s.id, s.title || 'this session')}</div></div>
    <span class="a-acct" title="${esc(s.profile)}">${esc(acctName(s.profile))}</span>${toolPill(s.harness)}${sync ? `<span class="a-pc" title="Ran on ${esc(device)}">${cdot(pcs, s.device, device, 'on')}<span>${esc(device)}</span></span>` : '<span></span>'}
    <span class="num">${fmt(s.tokens)}</span><span class="a-when ${s.open || far ? 'live' : ''}">${far ? 'running on ' + esc(far) : s.open ? 'live' : esc(when(s.last_active_at))}</span><span class="a-act">${act}</span></div>`;
}

view('sessions', async (sheet, params) => {
  // pc: every computer (''), this one ('here'), or one other computer (its id); all answered by the index
  const group = params.group || 'project', days = params.days || '7', live = params.live === '1', project = params.project || '', q = params.q || '', tag = params.tag || '', pc = params.pc || '', page = +params.page || 0;
  if (!S.state) await loadState();
  const scope = pc === 'here' ? 'here' : 'all';
  const filter = { harness: params.harness || null, profile: params.profile || null, days: days === 'all' || live ? null : +days, query: q || null, open: live || null, project: project || null, tag: tag || null, device: pc && pc !== 'here' ? pc : null, per: PER, page };
  sheet.className = 'sheet';
  // the list already on screen stays while the fresh one is read; from another page, a line says what is coming
  if (!sheet.querySelector('[data-tour=sessions]')) sheet.innerHTML = '<div class="page-body"><h1 class="muted">Reading sessions…</h1></div>';
  let [ans, plist, tlist, pcs] = await Promise.all([call('sessions', { filter, group, scope }), call('projects', { days: days === 'all' ? null : +days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  S.cache.tags = tlist;
  drawPins(plist);
  let groups = ans.groups;
  const total = ans.total, sync = syncOn(pcs);
  // a group opened in full fetches its own rows: the page carries at most 50 rows across every group
  const gfilter = (k) => ({ ...filter, per: 1000, page: 0, ...{ project: { project: k }, account: { profile: k }, tool: { harness: k }, tag: { tag: k }, date: { day: k } }[group] });
  await Promise.all(groups.filter((g) => sMore.has(group + ':' + g.key) && g.sessions.length < g.total).map(async (g) => {
    const r = await call('sessions', { filter: gfilter(g.key), group, scope }).catch(() => null);
    const full = r && r.groups.find((x) => x.key === g.key);
    if (full) g.sessions = full.sessions;
  }));
  const all = groups.flatMap((g) => g.sessions);
  S.cache.sessionOrder = all.map((s) => ({ profile: s.profile, id: s.id }));
  const tools = [...new Set(all.map((s) => s.harness))].map((h) => TOOL[h]).join(', ').replace(/, ([^,]*)$/, ' and $1');
  // a running session sits at the top, whatever the grouping, never inside a fold
  const isLive = (s) => s.open || s.open_elsewhere;
  const running = all.filter(isLive);
  if (!live) groups = groups.map((g) => ({ ...g, sessions: g.sessions.filter((s) => !isLive(s)), total: g.total - g.sessions.filter(isLive).length })).filter((g) => g.sessions.length || g.total);
  const filtered = !!(q || tag || live || pc || params.profile || params.harness);
  const cur = project ? plist.find((p) => p.key === project) || { key: project, name: (all[0] && all[0].project_name) || project, sessions: total, tokens: all.reduce((a, s) => a + s.tokens, 0), accounts: [...new Set(all.map((s) => s.profile))], live: running.length } : null;
  if (!total && !filtered && !cur && days === 'all') {
    patch(sheet, `<div class="page-body" data-tour="sessions"><section class="panel-card empty-state"><h1>No sessions yet.</h1><span class="muted">Sessions appear here as your tools write their logs.</span><div><button class="btn-primary" data-act="session-new" data-testid="session-new">New session</button></div></section></div>`);
    return;
  }
  const grp = (g, k, cls) => {
    const open = cls || !sFold.has(k), list = g.sessions, n = g.total || g.sessions.length;
    const p = group === 'project' && g.key ? plist.find((x) => x.key === g.key) : null;
    return `<section class="a-grp ${cls || ''} ${open ? '' : 'shut'}" data-key="grp:${esc(k)}" data-testid="group"><div class="a-gh"><h2 class="a-gname">${group === 'tag' && g.key && !cls ? `<span class="a-tag ${tagColor(g.key)}">${esc(g.name)}</span>` : esc(g.name)}</h2>${g.root ? `<span class="a-path mono" title="${esc(g.root)}">${esc(g.root)}</span>` : g.remote ? `<span class="a-path">${esc(g.remote)}</span>` : ''}
      ${group === 'project' && g.key && !cls ? `<button class="a-ib" data-act="project-menu" data-key="${esc(g.key)}" data-name="${esc(g.name)}" data-pinned="${p && p.pinned ? 1 : ''}" aria-label="Open, pin, rename or merge ${esc(g.name)}" aria-haspopup="menu">${ic.dots}</button>` : ''}
      <span class="a-tot">${plural(n, 'session')} · ${fmt(g.tokens)} tokens${p && p.cost != null ? ' · ' + money(p.cost) : ''}</span>${group === 'project' && g.root && !cls ? `<button class="btn-lane-start" data-act="project-new" data-dir="${esc(g.root)}" data-name="${esc(g.name)}" data-profile="${esc(g.sessions[0] ? g.sessions[0].profile : '')}" data-testid="project-new">New session</button>` : ''}${cls ? '' : `<button class="a-fold" data-act="s-fold" data-k="${esc(k)}" aria-expanded="${open}" aria-label="${open ? 'Fold' : 'Open'} ${esc(g.name)}">${open ? '▾' : '▸'}</button>`}</div>${open ? list.map((s) => sessionRow(s, pcs)).join('') + (list.length < n ? `<div class="a-more-row"><button class="filter-chip" data-act="s-more" data-k="${esc(k)}" data-testid="s-more-${esc(g.key)}">Show all ${plural(n, 'session')}</button></div>` : '') : ''}</section>`;
  };
  // the rail: what runs now, every project, the pinned ones, the recent ones, and the tags
  const keep = clean({ group: params.group, days: params.days });
  const tagItem = (t) => `<a class="a-rail-item ${tag === t.tag ? 'on' : ''}" href="${href('sessions', { ...keep, tag: t.tag })}" data-testid="rail-tag-${esc(t.tag)}"><i class="a-dot ghost"></i><span><span class="a-tag ${tagColor(t.tag)}">${esc(t.tag)}</span></span><span class="a-cnt">${t.sessions + t.projects}</span></a>`;
  const item = (p) => `<div class="a-rail-row" data-key="rail:${esc(p.key)}"><a class="a-rail-item ${project === p.key ? 'on' : ''}" href="${href('sessions', { ...keep, project: p.key })}" data-testid="rail-${esc(p.key)}"><i class="a-dot ${p.live ? 'live' : ''}"></i><span>${esc(p.name)}</span><span class="a-cnt ${p.live ? 'live' : ''}">${p.live ? p.live + ' live' : p.sessions}</span></a>${pinBtn(p)}</div>`;
  const pinned = plist.filter((p) => p.pinned), rest = plist.filter((p) => !p.pinned);
  const rail = `<aside class="a-rail" aria-label="Projects, pinned and recent" data-testid="rail" data-tour="rail">
      <a class="a-rail-item ${live ? 'on' : ''}" href="${href('sessions', { ...keep, live: '1' })}" data-testid="rail-live"><i class="a-dot live"></i><span>Live now</span><span class="a-cnt live">${running.length || ''}</span></a>
      <a class="a-rail-item ${!live && !project ? 'on' : ''}" href="${href('sessions', keep)}" data-testid="rail-all"><i class="a-dot"></i><span>All projects</span><span class="a-cnt">${plist.reduce((a, p) => a + p.sessions, 0)}</span></a>
      ${pinned.length ? `<div class="a-rail-sec">Pinned</div>${pinned.map(item).join('')}` : ''}
      <div class="a-rail-sec">${pinned.length ? 'Recent' : 'Projects'}</div>${(railMore ? rest : rest.slice(0, 8)).map(item).join('')}${rest.length > 8 && !railMore ? `<button class="a-rail-link muted" data-act="rail-more">${rest.length - 8} more…</button>` : ''}
      <a class="a-rail-link" href="#projects">Every project, with costs →</a>
      ${tlist.length ? `<div class="a-rail-sec" data-tour="tags">Tags</div><div class="a-rail-tags">${tlist.map(tagItem).join('')}</div>` : ''}</aside>`;
  const head = cur
    ? `<a class="filter-chip back-chip" href="#projects">← Projects</a><header class="page-heading"><div><h1 class="a-h1row" data-testid="project-title">${nameEl(cur)}<button class="a-ib" data-act="project-rename" data-key="${esc(cur.key)}" title="Rename (or double-click the name)" aria-label="Rename ${esc(cur.name)}">${ic.edit}</button>${pinBtn(cur)}${tagsHtml(cur.tags, 'project', cur.key, cur.name)}</h1><div class="heading-facts mono a-path">${esc(cur.remote || cur.root || '')}</div>
        <div class="a-pstats" data-testid="project-stats"><span><b>${money(cur.cost)}</b> ≈ list prices</span><span><b>${fmt(cur.tokens)}</b> tokens</span><span><b>${cur.sessions}</b> ${cur.sessions === 1 ? 'session' : 'sessions'}</span><span><b>${(cur.accounts || []).length}</b> ${(cur.accounts || []).length === 1 ? 'account' : 'accounts'}</span>${cur.live ? `<span class="a-live"><b>${cur.live}</b> running</span>` : ''}</div></div>
      <div class="page-actions"><button class="icon-btn" data-act="project-menu" data-key="${esc(cur.key)}" data-name="${esc(cur.name)}" data-pinned="${cur.pinned ? 1 : ''}" aria-label="Pin, rename or merge" aria-haspopup="menu">⋯</button>${cur.root ? `<button class="btn-primary" data-act="project-new" data-dir="${esc(cur.root)}" data-name="${esc(cur.name)}" data-profile="${esc(all[0] ? all[0].profile : '')}" data-testid="project-new">New session</button>` : ''}</div></header>`
    : `<header class="page-heading"><div><h1 data-testid="sessions-finding">${running.length ? `${plural(running.length, 'session is', 'sessions are')} running now; open one to follow it live.` : `Every tool's sessions in one place${tools ? ', from ' + esc(tools) : ''}.`}</h1><div class="heading-facts" data-testid="sessions-count">${plural(total, 'session')}${days !== 'all' ? ' · ' + days + ' days' : ''} · ${plural(groups.length, group === 'project' ? 'project' : 'group')}${sync ? ' · ' + plural(computers(pcs).length, 'computer') : ''} · tokens from the logs · cost ≈ list prices</div></div>
      <div class="page-actions"><button class="btn-primary" data-act="session-new" data-testid="session-new">New session</button></div></header>`;
  // a project's view has no rail of other projects: its own accounts and tags are chips in the bar, and a panel
  // about the project appears only when it spans several folders or accounts
  const folders = cur ? [...new Set(all.map((s) => s.cwd).filter(Boolean))] : [];
  const accts = cur ? (cur.accounts && cur.accounts.length ? cur.accounts : [...new Set(all.map((s) => s.profile))]) : [];
  const ptags = cur ? [...new Set([...(cur.tags || []), ...all.flatMap((s) => s.tags || [])])] : [];
  const about = cur && (folders.length > 1 || accts.length > 1) ? `<aside class="a-rail a-about" aria-label="This project" data-testid="project-about">
      <div class="a-rail-sec">This project</div><div class="a-kv"><b>${money(cur.cost)}</b><span>≈ list prices</span><b>${fmt(cur.tokens)}</b><span>tokens</span><b>${cur.sessions}</b><span>${cur.sessions === 1 ? 'session' : 'sessions'}</span></div>
      ${cur.series && cur.series.some((v) => v) ? `<div class="a-rail-sec">${days === 'all' ? 'Activity' : days + ' days'}</div>${area(cur.series)}` : ''}
      ${running.length ? `<div class="a-rail-sec">Running</div>${running.map((s) => `<a class="a-rail-item" href="#session?profile=${encodeURIComponent(s.profile)}&id=${encodeURIComponent(s.id)}"><i class="a-dot live"></i><span>${esc(s.title || 'Untitled session')}</span><span></span></a>`).join('')}` : ''}
      ${accts.length > 1 ? `<div class="a-rail-sec">Accounts</div>${accts.map((a) => { const n = all.filter((s) => s.profile === a).length; return `<div class="a-arow"><span>${esc(acctName(a))}</span><span>${n}${all.length ? ' · ' + Math.round((n / all.length) * 100) + '%' : ''}</span></div>`; }).join('')}` : ''}
      ${folders.length > 1 ? `<div class="a-rail-sec">Folders</div>${folders.slice(0, 6).map((f) => `<div class="a-afold mono" title="${esc(f)}">${esc(f)}</div>`).join('')}${folders.length > 6 ? `<div class="a-afold">${folders.length - 6} more</div>` : ''}` : ''}</aside>` : '';
  const fbar = `<div class="horizon-filterbar a-fbar" data-tour="filters">${live ? `<button class="a-chipx" data-act="s-set" data-k="live" data-v="" data-testid="chip-live">Running now ✕</button>` : ''}${tag && !cur ? `<button class="a-chipx" data-act="s-set" data-k="tag" data-v="" data-testid="chip-tag"><span class="a-tag ${tagColor(tag)}">${esc(tag)}</span> ✕</button>` : ''}${q ? `<button class="a-chipx" data-act="s-set" data-k="q" data-v="">“${esc(q)}” ✕</button>` : ''}
      ${seg('group', [['project', 'Project'], ['date', 'Date'], ['account', 'Account'], ['tool', 'Tool'], ['tag', 'Tag']].filter(([k]) => !(cur && k === 'project')), group, 'Group by', (v) => 'group-' + v)}${seg('days', [['7', '7d'], ['30', '30d'], ['all', 'All']], days, '', (v) => 'days-' + v)}
      ${sync ? `<span data-testid="pc-seg">${seg('pc', [['', 'All computers'], ['here', 'This computer'], ...(pcs.peers.length <= 3 ? pcs.peers.map((x) => [x.id, x.name]) : [])], pc, '', (v) => 'pc-' + (v || 'all'))}${pcs.peers.length > 3 ? `<button class="filter-chip ${pc && pc !== 'here' ? 'active' : ''}" data-act="s-pc-menu" aria-haspopup="menu" data-testid="pc-other">${esc(pc && pc !== 'here' ? nameOf(pcs, pc) : 'Elsewhere')} ▾</button>` : ''}</span>` : ''}
      ${cur && accts.length > 1 ? seg('profile', [['', 'All'], ...accts.map((a) => [a, acctName(a)])], params.profile || '', 'Accounts', (v) => 'pacct-' + (v || 'all')) : ''}${cur && ptags.length ? seg('tag', [['', 'All'], ...ptags.map((t) => [t, t])], tag, 'Tags', (v) => 'ptag-' + (v || 'all')) : ''}<span class="a-hint">Search above searches every session</span></div>`;
  const body = `${running.length && !live ? grp({ key: '', name: 'Running now', sessions: running, total: running.length, tokens: running.reduce((a, s) => a + s.tokens, 0) }, 'live', 'livegrp') : ''}
    ${groups.length ? groups.map((g) => grp(g, group + ':' + g.key)).join('') : running.length && !live ? '' : `<section class="a-grp"><div class="a-empty">No sessions match.<button class="filter-chip" data-act="s-clear">Clear filters</button></div></section>`}
    ${total > PER ? `<div class="a-pager" data-testid="s-pager"><button class="filter-chip" data-act="s-set" data-k="page" data-v="${page - 1}" ${page === 0 ? 'disabled' : ''} aria-label="Previous page" data-testid="page-prev">‹</button><span>${page * PER + 1}–${Math.min(total, (page + 1) * PER)} of ${total}</span><button class="filter-chip" data-act="s-set" data-k="page" data-v="${page + 1}" ${(page + 1) * PER >= total ? 'disabled' : ''} aria-label="Next page" data-testid="page-next">›</button></div>` : ''}`;
  patch(sheet, `<div class="page-body" data-tour="sessions">${head}<div class="a-wrap ${cur && !about ? 'one' : ''}">${cur ? about : rail}<div class="a-smain" data-tour="groups">${fbar}${body}</div></div></div>`);
  // the top bar's search is the index's: a pause in typing asks for the matching sessions
  const find = document.getElementById('find');
  if (q && !find.value.trim()) find.value = q; // searched from another page
  let timer;
  S.pageFind = (v) => { clearTimeout(timer); timer = setTimeout(() => { if ((v || '').trim() !== q) sq({ q: (v || '').trim(), page: '' }); }, 250); };
  S.pageFind(find.value); // typed while the page was being drawn (a redraw clears the page's search until it is set again)
});
action('s-set', (el) => {
  if (el.dataset.k === 'q') document.getElementById('find').value = el.dataset.v;
  sq({ [el.dataset.k]: el.dataset.v, ...(el.dataset.k === 'page' ? {} : { page: '' }) });
});
// from four other computers on, they are one chip that opens a menu with every one of them (the same pc parameter)
action('s-pc-menu', (el) => { const pcs = S.cache.computers, pc = S.route.params.pc || ''; openMenu(el, pcs.peers.map((x) => ({ html: cdot(pcs, x.id, x.name, 'on') + esc(x.name), on: x.id === pc, testid: 'pc-' + x.id, run: () => sq({ pc: x.id, page: '' }) }))); });
action('s-clear', () => { document.getElementById('find').value = ''; sq({ live: '', tag: '', pc: '', profile: '', harness: '', q: '', page: '' }); });
action('s-more', (el) => { sMore.add(el.dataset.k); render(); });
action('s-fold', (el) => { const k = el.dataset.k; if (sFold.has(k)) sFold.delete(k); else sFold.add(k); render(); });
action('rail-more', () => { railMore = true; render(); });
action('go-live', () => go('sessions', { live: '1' }));
action('go-apps', () => go('apps'));

// ---------- read a session ----------
// a header, five numbers, and the conversation as a chat with each turn's steps (folded) where they happened;
// Changes and Failed list only those steps. open: the steps unfolded; more: the long runs shown in full;
// follow: stay at the end of the conversation as a live session grows
const rv = { mode: 'all', open: new Set(), more: new Set(), sessionKey: '', follow: false };
const secsOf = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : ms + ' ms');
const diffHtml = (d) => `<div class="a-diff mono">${String(d || '').split('\n').map((l) => `<span class="${l[0] === '+' ? 'a-add' : l[0] === '-' ? 'a-del' : l.startsWith('@@') ? 'a-hunk' : ''}">${esc(l) || ' '}</span>`).join('')}</div>`;

view('session', async (sheet, params) => {
  sheet.className = 'sheet';
  const key = params.profile + '/' + params.id;
  // a redraw of the shown session (live update, unfolding a step) keeps it on screen until the new one is ready
  if (rv.sessionKey !== key || !sheet.querySelector('#chat')) sheet.innerHTML = '<div class="page-body"><h1 class="muted">Opening the session…</h1></div>';
  // a session from another computer: its transcript is fetched from the sync folder the first time
  if (params.device && rv.sessionKey !== key) toast('Fetching from the folder…');
  const [t, ss, tl] = await Promise.all([call('transcript', { profile: params.profile, id: params.id, device: params.device }), call('session_stats', { profile: params.profile, id: params.id }).catch(() => null), S.cache.tags ? null : call('tag_list').catch(() => [])]);
  if (tl) S.cache.tags = tl;
  if (!S.state) await loadState();
  if (rv.sessionKey !== key) { rv.sessionKey = key; rv.open = new Set(); rv.more = new Set(); rv.follow = !!t.meta.open; rv.mode = 'all'; }
  const m = t.meta, pcs = S.cache.computers;
  const steps = t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking');
  const changes = steps.filter((st) => st.change), failed = steps.filter((st) => st.status === 'error'), files = new Set(changes.map((st) => st.change.path)).size;
  const win = !S.state || !S.state.os || S.state.os === 'windows';
  const term = !win ? 'Terminal' : S.state.settings.terminal === 'console' ? 'a console window' : 'Windows Terminal';
  const tool = TOOL[m.harness] || m.harness;
  const project = m.project_name || (m.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder';
  // one step: ✓ / ✗ / …, the tool, what it worked on, its time, its change; it unfolds to its output or diff
  const row = (st, k) => {
    const open = rv.open.has(k), bad = st.status === 'error', busy = st.status === 'running' || st.status === 'pending';
    const dur = bad ? `failed${st.duration_ms ? ' · ' + secsOf(st.duration_ms) : ''}` : st.duration_ms ? secsOf(st.duration_ms) : busy ? 'running now' : '';
    return `<div class="a-step ${open ? 'open' : ''}" data-key="st:${k}"><button class="a-sb" data-act="step-open" data-k="${k}" aria-expanded="${open}"><span class="${bad ? 'a-bad' : busy ? 'a-busy' : 'a-ok'}">${bad ? '✗' : busy ? '…' : '✓'}</span><span class="a-k">${esc(st.tool)}</span><span class="a-what mono" title="${esc(st.input)}">${esc(st.input)}</span><span class="a-sdur ${bad ? 'a-bad' : ''}">${esc(dur)}</span><span class="a-sdiff">${st.change ? `+${st.change.added} −${st.change.removed}` : ''}</span></button>${open ? `<div class="a-sdet">${st.change ? diffHtml(st.change.diff) : `<pre class="a-sout mono">${esc(st.error || st.result || 'No output.')}</pre>`}</div>` : ''}</div>`;
  };
  // a run of steps; a long one (thousands in a Codex run) shows its first and last until asked for all
  const run = (list, id, head, tail) => !list.length ? '' : `<div class="a-steps" data-key="run:${id}">${rv.more.has(id) || list.length <= head + tail + 2 ? list.map(([st, k]) => row(st, k)).join('')
    : list.slice(0, head).map(([st, k]) => row(st, k)).join('') + `<div class="a-more" data-key="more:${id}"><button class="filter-chip active" data-act="more-steps" data-run="${id}" data-testid="more-steps">Show ${(list.length - head - tail).toLocaleString()} more steps</button></div>` + list.slice(-tail).map(([st, k]) => row(st, k)).join('')}</div>`;
  const who = (x) => `<span class="a-who">${esc(tool.toUpperCase())}${x.answer_at ? ' · ' + hm(x.answer_at) : ''}${x.model ? ' · ' + esc(x.model) : ''}</span>`;
  // All: each turn in order, the prompt, then the replies with the steps between them where they happened
  let k = 0;
  const chatAll = t.turns.map((x, ti) => {
    let out = x.prompt ? `<div class="a-msg you" data-key="you:${ti}"><span class="a-who">YOU${x.prompt_at ? ' · ' + hm(x.prompt_at) : ''}</span><div class="a-tx">${esc(x.prompt)}</div></div>` : '';
    const at = x.steps.map((st) => (st.kind === 'thinking' ? -1 : k++));
    const pos = x.answer.map((_, i) => (x.answer_pos && x.answer_pos[i] != null ? x.answer_pos[i] : x.steps.length));
    let from = 0;
    const flush = (to) => { const l = x.steps.slice(from, to).map((st, i) => [st, at[from + i]]).filter(([, j]) => j >= 0); out += run(l, ti + ':' + from, 5, 5); from = to; };
    // replies with no step between them share one bubble
    const groups = [];
    x.answer.forEach((a, i) => { if (!groups.length || groups[groups.length - 1].p !== pos[i]) groups.push({ p: pos[i], a: [] }); groups[groups.length - 1].a.push(a); });
    groups.forEach((g, gi) => { flush(Math.max(from, g.p)); out += `<div class="a-msg ai" data-key="ai:${ti}:${gi}">${who(x)}<div class="a-tx">${g.a.map(md).join('')}</div></div>`; });
    flush(x.steps.length);
    return out;
  }).join('');
  // Changes and Failed: only those steps, as one list
  const picked = steps.map((st, j) => [st, j]).filter(([st]) => (rv.mode === 'changes' ? st.change : st.status === 'error'));
  const body = rv.mode === 'all' ? chatAll : run(picked, 'flat', 30, 10) || `<div class="a-empty">${rv.mode === 'changes' ? 'This session changed no files.' : 'Nothing failed.'}</div>`;
  // archived: read only; from another computer: resume here, or why not; here: resume in the terminal of the setting
  const actions = m.archived ? '<span class="muted" data-testid="resume-reason">archived: it can be read, not resumed</span>'
    : params.device ? (canResume(m.resumable) ? `<button class="btn-primary" data-act="resume-here" data-s="${resumeData(m)}" data-testid="resume-here">Resume here</button>` : `<span class="muted" title="${esc(m.resumable || '')}" data-testid="resume-reason">${esc(reasonShort(m.resumable))}</span>`)
    : `<span class="a-split"><button class="btn-secondary" data-act="resume" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}" data-cwd="${esc(m.cwd)}" data-testid="resume">Resume in ${term}</button>${win ? `<button class="btn-secondary" data-act="terminal-menu" aria-label="Choose terminal" aria-haspopup="menu">▾</button>` : ''}</span>`;
  const stat = (title, value, cls = '') => `<div class="account-stat"><span>${title}</span><b class="${cls}">${value}</b><span></span></div>`;
  patch(sheet, `<div class="page-body a-sbody" data-tour="session"><a class="filter-chip back-chip" href="#sessions">← Sessions · ${esc(project)}</a>
    <header class="page-heading"><div><h1 data-testid="session-title">${esc(m.title || 'Untitled session')}</h1><div class="a-meta">${m.open ? `<span class="a-pill live"><i class="live-pulse-dot"></i>LIVE${rv.follow ? ' · following' : ''}</span>` : ''}${m.archived ? '<span class="a-pill arch" data-testid="session-archived">archived</span>' : ''}<span>${esc(acctName(m.profile))} · ${esc(tool)}</span>${tagsHtml(m.tags, 'session', m.profile + '/' + m.id, m.title || 'this session')}${m.model ? `<span>${esc(m.model)}</span>` : ''}
        ${params.device ? `<span data-testid="session-from">${cdot(pcs, params.device, m.device_name, 'on')} ran on ${esc(m.device_name || 'another computer')}</span>` : m.continued_from ? `<span data-testid="session-from">continued from ${esc(m.continued_from)}</span>` : ''}
        <span>started ${esc(when(m.started_at))} · ${plural(t.turns.length, 'turn')}</span><span>${fmt(m.tokens)} tokens${ss && ss.cost != null ? ' · ' + money(ss.cost) + ' ≈' : ''}</span></div></div>
      <div class="page-actions" data-tour="resume">${params.device ? '' : `<button class="btn-secondary" data-testid="export-md" data-act="export-md" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}">Export as Markdown</button>`}${actions}<button class="icon-btn" data-act="session-menu" data-path="${esc(m.source_path)}" data-cwd="${esc(m.cwd)}" aria-label="More" aria-haspopup="menu">⋯</button></div></header>
    <section class="account-stats" aria-label="Session readouts">${stat('Tokens', fmt(m.tokens))}${stat('Read from cache', ss && ss.requests ? Math.round(ss.hit * 100) + '%' : '—')}${stat('First reply p50', ss && ss.p50 != null ? (ss.p50 / 1000).toFixed(1) + ' s' : '—')}${stat('Failed commands', String(failed.length), failed.length ? 'a-neg' : '')}${stat('Files changed', String(files))}</section>
    <section class="panel-card a-conv" data-testid="conv-panel" data-tour="conversation"><div class="panel-head"><h2 class="panel-title">Conversation<span class="muted">${plural(t.turns.length, 'turn')} · ${steps.length.toLocaleString()} steps · ${files} changed ${files === 1 ? 'file' : 'files'} · ${failed.length} failed</span></h2>
        <div class="chip-group" role="group" aria-label="Show" data-tour="modes">${[['all', 'All'], ['changes', 'Changes'], ['failed', 'Failed']].map(([v, l]) => `<button class="filter-chip ${rv.mode === v ? 'active' : ''}" data-act="rv-mode" data-v="${v}" aria-pressed="${rv.mode === v}" data-testid="mode-${v}">${l}</button>`).join('')}</div></div>
      <div class="a-chat" id="chat" data-testid="chat" tabindex="0" aria-label="Conversation; scroll to read">${body || '<div class="a-empty">This session has no messages yet.</div>'}<div class="a-toend" id="toend" hidden><button class="btn-primary" data-act="follow-end" data-testid="follow-end">↓ Latest${m.open ? ' · follow live' : ''}</button></div></div></section></div>`);
  S.cache.transcript = t;
  // a live session opens at its end, following; a redraw keeps the reader's place (patch), or the end while following
  const chat = document.getElementById('chat');
  if (rv.follow) chat.scrollTop = chat.scrollHeight;
  const atEnd = () => chat.scrollHeight - chat.scrollTop - chat.clientHeight < 40;
  const show = () => { document.getElementById('toend').hidden = atEnd(); };
  chat.onscroll = () => { rv.follow = atEnd(); show(); };
  show();
  watchLive(m);
});

// an open session redraws while its tool writes to it: the log file's size and time are checked every 2 s
let liveTimer;
export function watchLive(m) {
  clearInterval(liveTimer);
  if (!m.open || !m.source_path) return;
  const key = rv.sessionKey;
  let last = null;
  liveTimer = setInterval(async () => {
    if (S.route.view !== 'session' || rv.sessionKey !== key) return clearInterval(liveTimer);
    let stamp;
    try { stamp = JSON.stringify(await call('file_stamp', { path: m.source_path })); } catch { return; }
    if (last === null) { last = stamp; return; }
    if (stamp === last || document.getElementById('dialog-root').innerHTML) return;
    last = stamp;
    render();
  }, 2000);
}
action('follow-end', () => {
  rv.follow = true;
  const chat = document.getElementById('chat');
  if (chat) chat.scrollTop = chat.scrollHeight;
});
action('more-steps', (el) => { rv.more.add(el.dataset.run); render(); });
action('step-open', (el) => { const k = +el.dataset.k; if (rv.open.has(k)) rv.open.delete(k); else rv.open.add(k); render(); });
action('rv-mode', (el) => { rv.mode = el.dataset.v; render(); });
action('terminal-menu', (el) => openMenu(el, [
  { label: 'Open sessions in Windows Terminal', run: () => setTerminal('wt') },
  { label: 'Open sessions in a console window', run: () => setTerminal('console') },
]));
const setTerminal = (v) => mutate(() => call('settings_set', { key: 'terminal', value: v }), 'Saved');
action('session-menu', (el) => openMenu(el, [{ label: 'Open the log file folder', run: () => call('open_folder', { path: el.dataset.path.replace(/[\\/][^\\/]*$/, '') }) }]));
action('export-md', async (el) => {
  const path = await saveFile(`${(S.cache.transcript.meta.title || 'session').replace(/[^\w -]+/g, '').slice(0, 60)}.md`);
  if (path) attempt(() => call('export_markdown', { profile: el.dataset.profile, id: el.dataset.id, path }), 'Exported');
});

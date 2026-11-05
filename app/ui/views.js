// Sessions, the reading view, usage, routing, MCP servers, skills, prompts, settings and the CLI reference.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, fit, openDialog, closeDialog, dlgHead, confirmDialog, openMenu, action, render, pickFolder, saveFile, pickFile, shownTools, applyTheme, drawPins, notice, warmPage, keepPage } from './core.js';
import { resetGuides } from './guide.js';
import { esc, fmt, hm, when, dur, avatar, toolPill, toolIcon, ic, md, diffHtml, bars } from './lib.js';
import { sessionRow } from './accounts.js';
import { computersStatus, syncOn, computers, nameOf, cdot, cdots, heardOn, transportLine } from './computers.js';
import { resumeData, reasonShort } from './resume.js';
import { welcomeBack } from './restore.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };

// ---------- tags ----------
// a tag's colour follows its place in the list (oldest first), so it never changes
const tagColor = (t) => 'c' + (Math.max(0, (S.cache.tags || []).findIndex((x) => x.tag === t)) % 6);
/** Tag chips for a session or project: each a link to that tag's list, plus + to edit when editable. */
export function tagsHtml(tags, kind, key, name, editable = true) {
  // a chip has its own action, so a click on one inside a row's title does not open the session
  const all = tags || [], shown = all.slice(0, 2), more = all.slice(2);
  const chip = (t) => `<a class="tagc ${tagColor(t)}" href="${href('sessions', { tag: t })}" data-act="tag-go" data-tag="${esc(t)}" title="Sessions tagged ${esc(t)}">${esc(t)}</a>`;
  return `<span class="tags">${editable ? `<button class="ib tagb" data-act="tag-edit" data-kind="${kind}" data-key="${esc(key)}" data-name="${esc(name)}" data-tags="${esc(JSON.stringify(all))}" title="${all.length ? `${all.length} ${all.length === 1 ? 'tag' : 'tags'}: ${esc(all.join(', '))}` : 'Add a tag'}" aria-label="Tags of ${esc(name)}">${ic.tag}${all.length ? `<small>${all.length}</small>` : ''}</button>` : ''}${shown.map(chip).join('')}${more.length ? `<span class="tagc more" title="${esc(more.join(', '))}">+${more.length}</span>` : ''}</span>`;
}
action('tag-go', (el) => go('sessions', { tag: el.dataset.tag }));
action('tag-edit', async (el) => {
  const { kind, key, name } = el.dataset;
  const mine = new Set(JSON.parse(el.dataset.tags || '[]'));
  const known = await attempt(() => call('tag_list')) || [];
  S.cache.tags = known;
  for (const t of mine) if (!known.some((k) => k.tag === t)) known.push({ tag: t, sessions: 0, projects: 0 });
  const row = (t) => `<label class="tagrow"><input type="checkbox" value="${esc(t.tag)}" ${mine.has(t.tag) ? 'checked' : ''}> <span class="tagc ${tagColor(t.tag)}">${esc(t.tag)}</span><span class="muted mono sm">${t.sessions + t.projects || ''}</span></label>`;
  openDialog(`${dlgHead('Tags of ' + name)}
    <div class="df" style="grid-template-columns:1fr"><div><input class="input" id="tg-new" placeholder="Type a tag and press Enter to add it" aria-label="New tag" data-testid="tag-new"><span class="hint">Tags are yours: free text, shared across tools and accounts, never written into the tools' files.${kind === 'project' ? ' A project\'s tag reaches every session of it in filters.' : ''}</span></div></div>
    <div class="df" style="grid-template-columns:1fr"><div class="taglist" id="tg-list" data-testid="tag-list">${known.map(row).join('') || '<span class="muted">No tags yet.</span>'}</div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="tg-ok" data-testid="tag-save">Save</button></span></div>`);
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

// ---------- sessions and projects ----------
// the list's state lives in the hash (#sessions?group=date&days=30&live=1&project=<key>&q=...), so every filter is a link
const clean = (p) => Object.fromEntries(Object.entries(p).filter(([, v]) => v !== '' && v != null));
const sq = (patch) => go('sessions', clean({ ...S.route.params, ...patch }));
const href = (view, p) => '#' + view + (Object.keys(clean(p)).length ? '?' + new URLSearchParams(clean(p)) : '');
const money = (c, d = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d }));
const sMore = new Set();
let railMore = false;
// the page reads the index in pages of 50 rows; the search, the computer and every filter are the backend's
const PER = 50;

view('sessions', async (sheet, params) => {
  // pc: every computer (''), this one ('here'), or one other computer (its id); all answered by the index
  const group = params.group || 'project', days = params.days || '7', live = params.live === '1', project = params.project || '', q = params.q || '', tag = params.tag || '', pc = params.pc || '', page = +params.page || 0;
  if (!S.state) await call('state').then((s) => (S.state = s));
  const scope = pc === 'here' ? 'here' : 'all';
  const filter = { harness: params.harness || null, profile: params.profile || null, days: days === 'all' ? null : +days, query: q || null, open: live || null, project: project || null, tag: tag || null, device: pc && pc !== 'here' ? pc : null, per: PER, page };
  sheet.className = 'sheet';
  const wk = 'sessions:' + JSON.stringify(params);
  if (!warmPage(sheet, wk)) sheet.innerHTML = `<div class="crumb">Sessions</div><h1>Reading sessions…</h1>`;
  let [ans, plist, tlist, pcs] = await Promise.all([call('sessions', { filter, group, scope }), call('projects', { days: days === 'all' ? null : +days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  S.cache.tags = tlist;
  drawPins(plist);
  let groups = ans.groups;
  const total = ans.total;
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
  const running = all.filter((s) => s.open);
  if (!live) groups = groups.map((g) => ({ ...g, sessions: g.sessions.filter((s) => !s.open) })).filter((g) => g.sessions.length);
  const cur = project ? plist.find((p) => p.key === project) || { key: project, name: (all[0] && all[0].project_name) || project, sessions: total, tokens: all.reduce((a, s) => a + s.tokens, 0), accounts: [...new Set(all.map((s) => s.profile))], live: running.length } : null;
  const grp = (g, k, cls) => {
    const open = !sFold.has(k), list = g.sessions, n = g.total || g.sessions.length;
    const cost = g.key && plist.find((p) => p.key === g.key && p.cost != null);
    return `<div class="grp ${cls || ''} ${open ? '' : 'shut'}" data-testid="group"><div class="gh"><h2 class="gname">${group === 'tag' && g.key ? `<span class="tagc ${tagColor(g.key)}">${esc(g.name)}</span>` : esc(g.name)}</h2>${g.root && !g.remote ? `<span class="mono">${esc(g.root)}</span>` : ''}${g.remote ? `<span class="rem">${esc(g.remote)}</span>` : ''}
      ${group === 'project' && !cls ? `<button class="iconbtn" style="width:24px;height:24px" data-act="project-menu" data-key="${esc(g.key)}" data-name="${esc(g.name)}" data-pinned="${cost && cost.pinned || (plist.find((p) => p.key === g.key) || {}).pinned ? 1 : ''}" aria-label="Open, pin, rename or merge">${ic.dots}</button>` : ''}
      <span class="tot mono">${n} ${n === 1 ? 'session' : 'sessions'} · ${fmt(g.tokens)} tokens${cost && group === 'project' ? ' · ' + money(cost.cost) : ''}</span>${group === 'project' && g.root && !cls ? `<button class="btn outline sm" data-act="project-new" data-dir="${esc(g.root)}" data-name="${esc(g.name)}" data-profile="${esc(g.sessions[0] ? g.sessions[0].profile : '')}" data-testid="project-new">${ic.play} New session</button>` : ''}<button class="fold" data-act="s-fold" data-k="${esc(k)}" aria-expanded="${open}" aria-label="${open ? 'Fold' : 'Open'} ${esc(g.name)}"><span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></button></div>${open ? list.map(sessionRow).join('') + (list.length < n ? `<div class="srow more"><span></span><a class="acT" data-act="s-more" data-k="${esc(k)}" role="button" tabindex="0" data-testid="s-more-${esc(g.key)}">Show all ${n} sessions</a></div>` : '') : ''}</div>`;
  };
  // the rail: what runs now, every project, the pinned ones, the recent ones, and the tags
  const keep = clean({ group: params.group, days: params.days });
  const tagItem = (t) => `<a class="${tag === t.tag ? 'on' : ''}" href="${href('sessions', { ...keep, tag: t.tag })}" data-testid="rail-tag-${esc(t.tag)}"><span class="tagc ${tagColor(t.tag)}">${esc(t.tag)}</span><span class="cnt">${t.sessions + t.projects}</span></a>`;
  const item = (p) => `<a class="${project === p.key ? 'on' : ''}" href="${href('sessions', { ...keep, project: p.key })}" data-testid="rail-${esc(p.key)}"><i class="dot ${p.live ? 'live' : ''}"></i><span>${esc(p.name)}</span><span class="cnt ${p.live ? 'live' : ''}">${p.live ? p.live + ' live' : p.sessions}</span></a>`;
  const pinned = plist.filter((p) => p.pinned), rest = plist.filter((p) => !p.pinned);
  const rail = `<aside class="panel rail" data-testid="rail">
      <a class="${live ? 'on' : ''}" href="${href('sessions', { ...keep, live: '1' })}" data-testid="rail-live"><i class="dot live"></i><span>Live now</span><span class="cnt live">${running.length || ''}</span></a>
      <a class="${!live && !project ? 'on' : ''}" href="${href('sessions', keep)}" data-testid="rail-all"><i class="dot"></i><span>All projects</span><span class="cnt">${plist.reduce((a, p) => a + p.sessions, 0)}</span></a>
      ${pinned.length ? `<div class="sec">Pinned</div>${pinned.map(item).join('')}` : ''}
      <div class="sec">${pinned.length ? 'Recent' : 'Projects'}</div>${(railMore ? rest : rest.slice(0, 8)).map(item).join('')}${rest.length > 8 && !railMore ? `<a data-act="rail-more" role="button" tabindex="0"><span class="muted">${rest.length - 8} more…</span></a>` : ''}
      <a href="#projects" class="all"><span class="acT">Every project, with costs →</span></a>
      ${tlist.length ? `<div class="sec">Tags</div>${tlist.map(tagItem).join('')}` : ''}</aside>`;
  const head = cur
    ? `<div class="trow sesshead"><div><a href="#projects" class="crumb acT">← Projects</a><h1 data-testid="project-title">${nameEl(cur, true)}<button class="ib edit" data-act="project-rename" data-key="${esc(cur.key)}" title="Rename (or double-click the name)" aria-label="Rename ${esc(cur.name)}">${ic.edit}</button>${pinBtn(cur)}${tagsHtml(cur.tags, 'project', cur.key, cur.name)}</h1><p class="find mono">${esc(cur.remote || cur.root || '')}</p>
        <div class="pstats" data-testid="project-stats"><span><b>${money(cur.cost)}</b> at list prices</span><span><b>${fmt(cur.tokens)}</b> tokens</span><span><b>${cur.sessions}</b> ${cur.sessions === 1 ? 'session' : 'sessions'}</span><span><b>${(cur.accounts || []).length}</b> ${(cur.accounts || []).length === 1 ? 'account' : 'accounts'}</span>${cur.live ? `<span class="acT"><b>${cur.live}</b> running</span>` : ''}</div></div>
      <div class="acts"><button class="iconbtn sq" data-act="project-menu" data-key="${esc(cur.key)}" data-name="${esc(cur.name)}" data-pinned="${cur.pinned ? 1 : ''}" aria-label="Pin, rename or merge">${ic.dots}</button>
        ${cur.root ? `<button class="btn primary" data-act="project-new" data-dir="${esc(cur.root)}" data-name="${esc(cur.name)}" data-profile="${esc(all[0] ? all[0].profile : '')}" data-testid="project-new">${ic.play} New session</button>` : ''}</div></div>`
    : `<div class="trow sesshead"><div><div class="crumb">Sessions${days !== 'all' ? ' · ' + days + ' days' : ''}${syncOn(pcs) ? ' · ' + computers(pcs).length + ' computers' : ''}</div><h1 data-testid="sessions-finding">${running.length ? `<span class="w">${running.length} ${running.length === 1 ? 'session is' : 'sessions are'} running now</span>; open one to follow it live.` : `Every tool's sessions in one place${tools ? ', from ' + esc(tools) : ''}.`}</h1><p class="find">${total} ${total === 1 ? 'session' : 'sessions'}${days !== 'all' ? ' in ' + days + ' days' : ''} across ${groups.length} ${group === 'project' ? (groups.length === 1 ? 'project' : 'projects') : 'groups'}. Tokens are counted from the logs; costs are at list prices, so ≈.</p></div>
      <div class="acts"><span class="muted mono sm" data-testid="sessions-count">${total} ${total === 1 ? 'session' : 'sessions'}${days !== 'all' ? ' · ' + days + ' days' : ''} · ${groups.length} ${group === 'project' ? (groups.length === 1 ? 'project' : 'projects') : 'groups'}</span>
      <button class="btn primary" data-act="session-new" data-testid="session-new">${ic.play} New session</button></div></div>`;
  // a project's view has no rail of other projects: its own accounts and tags are chips in the bar, and a panel
  // about the project appears only when it spans several folders or accounts
  const folders = cur ? [...new Set(all.map((s) => s.cwd).filter(Boolean))] : [];
  const accts = cur ? (cur.accounts && cur.accounts.length ? cur.accounts : [...new Set(all.map((s) => s.profile))]) : [];
  const ptags = cur ? [...new Set([...(cur.tags || []), ...all.flatMap((s) => s.tags || [])])] : [];
  const about = cur && (folders.length > 1 || accts.length > 1) ? `<aside class="panel about" data-testid="project-about">
      <div class="sec">This project</div><div class="kv mono"><b>${money(cur.cost)}</b><span>at list prices</span><b>${fmt(cur.tokens)}</b><span>tokens</span><b>${cur.sessions}</b><span>${cur.sessions === 1 ? 'session' : 'sessions'}</span></div>
      ${cur.series && cur.series.some((v) => v) ? `<div class="sec">${days === 'all' ? 'Activity' : days + ' days'}</div>${area(cur.series)}` : ''}
      ${running.length ? `<div class="sec">Running</div>${running.map((s) => `<a class="arun" href="#session?profile=${encodeURIComponent(s.profile)}&id=${encodeURIComponent(s.id)}"><i class="dot live"></i><span>${esc(s.title || 'Untitled session')}</span></a>`).join('')}` : ''}
      ${accts.length > 1 ? `<div class="sec">Accounts</div>${accts.map((a) => { const n = all.filter((s) => s.profile === a).length; return `<div class="arow"><span>${esc(acctName(a))}</span><span class="mono muted">${n}${all.length ? ' · ' + Math.round((n / all.length) * 100) + '%' : ''}</span></div>`; }).join('')}` : ''}
      ${folders.length > 1 ? `<div class="sec">Folders</div>${folders.slice(0, 6).map((f) => `<div class="mono afold" title="${esc(f)}">${esc(f)}</div>`).join('')}${folders.length > 6 ? `<div class="muted sm">${folders.length - 6} more</div>` : ''}` : ''}</aside>` : '';
  const chips = cur ? `${accts.length > 1 ? `<span class="lbl">Accounts</span><span class="seg3">${[['', 'All'], ...accts.map((a) => [a, acctName(a)])].map(([v, l]) => `<a class="${(params.profile || '') === v ? 'on' : ''}" data-act="s-set" data-k="profile" data-v="${esc(v)}" data-testid="pacct-${esc(v || 'all')}">${esc(l)}</a>`).join('')}</span>` : ''}${ptags.length ? `<span class="lbl">Tags</span><span class="seg3">${[['', 'All'], ...ptags.map((t) => [t, t])].map(([v, l]) => `<a class="${tag === v ? 'on' : ''}" data-act="s-set" data-k="tag" data-v="${esc(v)}">${esc(l)}</a>`).join('')}</span>` : ''}` : '';
  sheet.innerHTML = `${head}
    <div class="swrap ${cur ? 'pv' : ''} ${about ? 'with' : ''}">${cur ? about : rail}<div class="smain">
    <div class="fbar sbar">${live ? `<a class="chip on" data-act="s-set" data-k="live" data-v="" role="button" tabindex="0" data-testid="chip-live">Running now ✕</a>` : ''}${tag && !cur ? `<a class="chip on" data-act="s-set" data-k="tag" data-v="" role="button" tabindex="0" data-testid="chip-tag"><span class="tagc ${tagColor(tag)}">${esc(tag)}</span> ✕</a>` : ''}${q ? `<a class="chip on" data-act="s-set" data-k="q" data-v="" role="button" tabindex="0">“${esc(q)}” ✕</a>` : ''}
      <span class="lbl">Group by</span><span class="seg3">${[['project', 'Project'], ['date', 'Date'], ['account', 'Account'], ['tool', 'Tool'], ['tag', 'Tag']].filter(([k]) => !(cur && k === 'project')).map(([k, l]) => `<a class="${group === k ? 'on' : ''}" data-act="s-set" data-k="group" data-v="${k}" data-testid="group-${k}">${l}</a>`).join('')}</span>
      <span class="seg3">${[['7', '7d'], ['30', '30d'], ['all', 'All']].map(([v, l]) => `<a class="${days === v ? 'on' : ''} mono" data-act="s-set" data-k="days" data-v="${v}" data-testid="days-${v}">${l}</a>`).join('')}</span>
      ${syncOn(pcs) ? `<span class="seg3" role="group" aria-label="Computer" data-testid="pc-seg">${[['', 'All computers'], ['here', 'This computer'], ...pcs.peers.map((x) => [x.id, x.name])].map(([v, l]) => `<a class="${pc === v ? 'on' : ''}" data-act="s-set" data-k="pc" data-v="${esc(v)}" data-testid="pc-${esc(v || 'all')}">${esc(l)}</a>`).join('')}</span>` : ''}
      ${chips}<span class="muted sm">Search above searches every session</span></div>
    ${running.length && !live ? grp({ name: 'Running now', sessions: running, tokens: running.reduce((a, s) => a + s.tokens, 0) }, 'live', 'livegrp') : ''}
    ${groups.length ? groups.map((g) => grp(g, group + ':' + g.key)).join('') : running.length ? '' : '<p class="empty">No sessions match.</p>'}
    ${total > PER ? `<div class="pager" data-testid="s-pager"><button class="btn sm" data-act="s-set" data-k="page" data-v="${page - 1}" ${page === 0 ? 'disabled' : ''} aria-label="Previous page" data-testid="page-prev">‹</button><span class="help">${page * PER + 1}–${Math.min(total, (page + 1) * PER)} of ${total}</span><button class="btn sm" data-act="s-set" data-k="page" data-v="${page + 1}" ${(page + 1) * PER >= total ? 'disabled' : ''} aria-label="Next page" data-testid="page-next">›</button></div>` : ''}</div></div>`;
  // the top bar's search is the index's: a pause in typing asks for the matching sessions
  const find = document.getElementById('find');
  if (q && !find.value.trim()) find.value = q; // searched from another page
  let timer;
  S.pageFind = (v) => { clearTimeout(timer); timer = setTimeout(() => { if ((v || '').trim() !== q) sq({ q: (v || '').trim(), page: '' }); }, 250); };
  S.pageFind(find.value); // typed while the page was being drawn (a redraw clears the page's search until it is set again)
  keepPage(sheet, wk);
});
action('s-set', (el) => {
  if (el.dataset.k === 'q') document.getElementById('find').value = el.dataset.v;
  sq({ [el.dataset.k]: el.dataset.v, ...(el.dataset.k === 'page' ? {} : { page: '' }) });
});
action('s-more', (el) => { sMore.add(el.dataset.k); render(); });
action('rail-more', () => { railMore = true; render(); });
action('go-live', () => go('sessions', { live: '1' }));
action('project-pin', (el) => mutate(() => call('project_pin', { key: el.dataset.key, on: el.dataset.on === '1' }), el.dataset.on === '1' ? 'Pinned to the sidebar' : 'Unpinned'));

// ---------- projects: every project as a row card, with its tokens, cost, sessions and activity ----------
// the page's state is in the hash (#projects?days=30&sort=cost&metric=cost&per=25&page=2); a choice redraws only
// the part it changes, through replaceState, so nothing else on the page moves
const SH = ['var(--ac)', 'var(--h3)', 'var(--h2)', 'var(--h1)', 'var(--iris)', 'var(--sand)'];
const pj = { list: null, fetched: '', days: '7', sort: 'active', metric: 'tokens', per: 10, page: 0, tiles: {} };
const pjHash = () => history.replaceState(null, '', href('projects', { days: pj.days === '7' ? '' : pj.days, sort: pj.sort === 'active' ? '' : pj.sort, metric: pj.metric === 'tokens' ? '' : pj.metric, per: pj.per === 10 ? '' : String(pj.per), page: pj.page ? String(pj.page) : '' }));
async function fetchProjects() {
  const [list, tl] = await Promise.all([call('projects', { days: pj.days === 'all' ? null : +pj.days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  pj.list = list; pj.fetched = pj.days; S.cache.tags = tl;
  drawPins(list);
}
const area = (s) => {
  const w = 300, h = 44, m = Math.max(1, ...s), n = Math.max(2, s.length);
  const pts = s.map((v, i) => [i * (w / (n - 1)), h - 4 - (v / m) * (h - 8)]);
  const d = pts.map((q, i) => (i ? 'L' : 'M') + q[0].toFixed(1) + ' ' + q[1].toFixed(1)).join(' ');
  return `<svg class="area" viewBox="0 0 ${w} ${h}" preserveAspectRatio="none" aria-hidden="true"><path d="${d} L${pts[pts.length - 1][0].toFixed(1)} ${h} L0 ${h}Z" fill="var(--ac-soft)"/><path d="${d}" fill="none" stroke="var(--ac)" stroke-width="1.5" vector-effect="non-scaling-stroke"/><circle cx="${pts[pts.length - 1][0].toFixed(1)}" cy="${pts[pts.length - 1][1].toFixed(1)}" r="2.5" fill="var(--ac)"/></svg>`;
};
const pinBtn = (p) => `<button class="ib pin ${p.pinned ? 'on' : ''}" data-act="project-pin" data-key="${esc(p.key)}" data-on="${p.pinned ? '' : '1'}" title="${p.pinned ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar'}" aria-label="${p.pinned ? 'Unpin' : 'Pin'} ${esc(p.name)}" data-testid="pin-${esc(p.key)}">${ic.pin}</button>`;
const editBtn = (p) => `<button class="ib edit" data-act="project-rename" data-key="${esc(p.key)}" title="Rename (or double-click the name)" aria-label="Rename ${esc(p.name)}" data-testid="rename-${esc(p.key)}">${ic.edit}</button>`;
/** With sync on: a dot per computer that has the repository (the counts above sum them all), and whether this one has it. */
const pcDots = (p) => {
  const pcs = S.cache.computers;
  if (!syncOn(pcs) || !p.devices || !p.devices.length) return '';
  return `<br><span class="pcs-row" title="${esc(p.devices.map((d) => `${d.name || nameOf(pcs, d.id)}: ${d.sessions} ${d.sessions === 1 ? 'session' : 'sessions'}`).join(', '))}" data-testid="pcard-pcs">${cdots(pcs, p.devices.map((d) => d.id))}${p.here === false ? '<span class="muted">not cloned here</span>' : ''}</span>`;
};
/** The name of a project, renamed in place by the pencil or a double-click. */
export const nameEl = (p, big) => `<span class="pname ${big ? 'big' : ''}" data-name="${esc(p.key)}" title="Double-click to rename">${esc(p.name)}</span>`;

view('projects', async (sheet, params) => {
  pj.days = params.days || '7'; pj.sort = params.sort || 'active'; pj.metric = params.metric || 'tokens'; pj.per = +params.per || 10; pj.page = +params.page || 0;
  sheet.className = 'sheet';
  if (!pj.list || pj.fetched !== pj.days) {
    if (!sheet.querySelector('#pj-rows')) sheet.innerHTML = `<div class="crumb">Projects</div><h1>Reading projects…</h1>`;
    await fetchProjects();
  }
  sheet.innerHTML = `<div class="trow"><div><div class="crumb">Projects · ${pj.days === 'all' ? 'all time' : pj.days + ' days'}</div><h1>${pj.list.length} ${pj.list.length === 1 ? 'project' : 'projects'} ran in ${pj.days === 'all' ? 'all time' : 'the last ' + pj.days + ' days'}${pj.list.filter((p) => p.live).length ? `; <span class="w">${pj.list.filter((p) => p.live).length}</span> ${pj.list.filter((p) => p.live).length === 1 ? 'has' : 'have'} a session open now` : ''}.</h1><p class="find">A project is a folder your tools ran in: click one for its sessions; pin, tag or rename it from its icons. Cost is at API list prices, so ≈.</p></div>
      <div class="acts"><span class="seg3">${[['7', '7d'], ['30', '30d'], ['all', 'All']].map(([v, l]) => `<a class="${pj.days === v ? 'on' : ''} mono" data-act="p-set" data-k="days" data-v="${v}" data-testid="pdays-${v}">${l}</a>`).join('')}</span>
        <span class="seg3" id="pj-sort"></span></div></div>
    <section class="ovkpis acctk" id="pj-tiles" data-testid="project-tiles"></section>
    <div id="pj-dist"></div>
    <div class="pcrows" id="pj-rows" data-testid="project-cards"></div>
    <div class="pager" id="pj-pager"></div>`;
  drawTiles(); drawDist(); drawRows();
});

/** The five tiles; a number that changed since the last draw flashes once. */
function drawTiles() {
  const el = document.getElementById('pj-tiles'); if (!el) return;
  const list = pj.list, tokens = list.reduce((a, p) => a + p.tokens, 0), sessions = list.reduce((a, p) => a + p.sessions, 0), live = list.reduce((a, p) => a + p.live, 0);
  const priced = list.filter((p) => p.cost != null), cost = priced.length ? priced.reduce((a, p) => a + p.cost, 0) : null;
  const nDays = pj.days === 'all' ? Math.max(1, ...list.map((p) => p.series.length)) : +pj.days;
  const busiest = [...list].sort((a, b) => b.tokens - a.tokens)[0];
  const t = { tokens: fmt(tokens), sessions: `${sessions} ${sessions === 1 ? 'session' : 'sessions'}`, cost: money(cost), day: cost != null ? money(cost / nDays) + ' a day' : 'no priced model yet', n: String(list.length), accounts: `${[...new Set(list.flatMap((p) => p.accounts))].length} accounts`, live: String(live), livenote: live ? list.filter((p) => p.live).map((p) => p.name).join(', ') : 'nothing running', busy: busiest ? busiest.name : '—', busynote: busiest && tokens ? Math.round((busiest.tokens / tokens) * 100) + '% of tokens' + (busiest.cost != null && cost ? ' · ' + Math.round((busiest.cost / cost) * 100) + '% of cost' : '') : '' };
  const was = pj.tiles, flash = (k) => (was[k] !== undefined && was[k] !== t[k] ? 'flash' : '');
  pj.tiles = t;
  el.innerHTML = `<div class="ovk"><div class="kl"><span>Tokens · ${pj.days === 'all' ? 'all time' : pj.days + ' days'}</span></div><strong class="${flash('tokens')}">${t.tokens}</strong><div class="kn">${t.sessions}</div></div>
    <div class="ovk"><div class="kl"><span>At list prices</span></div><strong class="${flash('cost')}">${t.cost}</strong><div class="kn">${t.day}</div></div>
    <div class="ovk"><div class="kl"><span>Projects</span></div><strong class="${flash('n')}">${t.n}</strong><div class="kn">${t.accounts}</div></div>
    <div class="ovk ${live ? 'click' : ''}" ${live ? 'data-href="#sessions?live=1"' : ''}><div class="kl"><span>Running now</span></div><strong class="${live ? 'acT' : ''} ${flash('live')}">${t.live}</strong><div class="kn" title="${esc(t.livenote)}">${esc(t.livenote)}</div></div>
    <div class="ovk"><div class="kl"><span>Busiest</span></div><strong style="font-size:20px" class="${flash('busy')}">${esc(t.busy)}</strong><div class="kn">${esc(t.busynote)}</div></div>`;
}

/** The distribution: a stacked bar and the top five rows, by tokens, cost or sessions; the tabs redraw only this. */
function drawDist() {
  const el = document.getElementById('pj-dist'); if (!el) return;
  const list = pj.list, metric = pj.metric;
  if (!list.length) { el.innerHTML = ''; return; }
  const val = (p) => (metric === 'cost' ? p.cost || 0 : metric === 'sessions' ? p.sessions : p.tokens);
  const total = list.reduce((a, p) => a + val(p), 0) || 1;
  const top = [...list].sort((a, b) => val(b) - val(a)), head5 = top.slice(0, 5), others = top.slice(5);
  const oVal = others.reduce((a, p) => a + val(p), 0), oCost = others.some((p) => p.cost != null) ? others.reduce((a, p) => a + (p.cost || 0), 0) : null;
  const on = (k) => (metric === k ? 'on' : '');
  const row = (name, key, v, cost, tokens, sessions, color, i) => `<div class="prow"><span class="sw" style="background:${color}"></span>${key ? `<a class="nm" href="${href('sessions', { project: key })}">${esc(name)}</a>` : `<span class="nm muted">${esc(name)}</span>`}<div class="track"><i style="width:${(v / total) * 100}%;${i === 'o' ? 'background:var(--ctl)' : ''}"></i></div><span class="num ${on('cost')}">${money(cost)}</span><span class="num ${on('tokens')}">${fmt(tokens)}</span><span class="num ${on('sessions')}">${sessions}</span><span class="m">${Math.round((v / total) * 100)}%</span></div>`;
  el.innerHTML = `<section class="panel" data-testid="project-dist"><div class="ph"><h2>Where the tokens and the cost went</h2><span class="seg3">${[['tokens', 'Tokens'], ['cost', 'Cost'], ['sessions', 'Sessions']].map(([v, l]) => `<a class="${on(v)}" data-act="p-set" data-k="metric" data-v="${v}" data-testid="pmetric-${v}">${l}</a>`).join('')}</span></div>
    <div class="stackbar" aria-hidden="true">${head5.map((p, i) => `<i style="width:${(val(p) / total) * 100}%;background:${SH[i]}" title="${esc(p.name)}"></i>`).join('')}${oVal ? `<i style="width:${(oVal / total) * 100}%;background:var(--ctl)"></i>` : ''}</div>
    <div class="prow head"><span></span><span>Project</span><span>Share of ${metric}</span><span class="num ${on('cost')}">Cost</span><span class="num ${on('tokens')}">Tokens</span><span class="num ${on('sessions')}">Sessions</span><span class="m">Share</span></div>
    ${head5.map((p, i) => row(p.name, p.key, val(p), p.cost, p.tokens, p.sessions, SH[i], i)).join('')}
    ${others.length ? row(`${others.length} other ${others.length === 1 ? 'project' : 'projects'}`, '', oVal, oCost, others.reduce((a, p) => a + p.tokens, 0), others.reduce((a, p) => a + p.sessions, 0), 'var(--ctl)', 'o') : ''}</section>`;
}

/** The rows of the current page, sorted; the sort and the pager redraw only this. */
function drawRows() {
  const el = document.getElementById('pj-rows'), pager = document.getElementById('pj-pager'), sortEl = document.getElementById('pj-sort');
  if (!el) return;
  const list = pj.list;
  if (sortEl) sortEl.innerHTML = [['active', 'Active'], ['cost', 'Cost'], ['tokens', 'Tokens'], ['name', 'Name']].map(([v, l]) => `<a class="${pj.sort === v ? 'on' : ''}" data-act="p-set" data-k="sort" data-v="${v}" data-testid="psort-${v}">${l}</a>`).join('');
  const sorted = [...list].sort((a, b) => pj.sort === 'cost' ? (b.cost || 0) - (a.cost || 0) : pj.sort === 'tokens' ? b.tokens - a.tokens : pj.sort === 'name' ? a.name.localeCompare(b.name) : b.last_active_at - a.last_active_at);
  const pages = Math.max(1, Math.ceil(sorted.length / pj.per));
  if (pj.page >= pages) pj.page = pages - 1;
  const shown = sorted.slice(pj.page * pj.per, pj.page * pj.per + pj.per);
  el.innerHTML = shown.map((p) => `<div class="panel pcrow ${p.live ? 'running' : ''}" data-act="project-open" data-key="${esc(p.key)}" role="link" tabindex="0" data-testid="pcard-${esc(p.key)}">
      <div class="who"><div class="hd">${nameEl(p)}${editBtn(p)}</div><span class="path mono" title="${esc(p.remote || p.root)}">${esc(p.remote || p.root)}</span><div class="hd">${tagsHtml(p.tags, 'project', p.key, p.name)}</div></div>
      ${area(p.series)}
      <div class="num"><b>${money(p.cost)}</b><small>${fmt(p.tokens)} tokens${p.cost != null && p.priced_share < 0.95 ? ' · ' + Math.round(p.priced_share * 100) + '% priced' : ''}</small></div>
      <div class="meta">${p.sessions} ${p.sessions === 1 ? 'session' : 'sessions'} · ${p.accounts.length} ${p.accounts.length === 1 ? 'account' : 'accounts'}<br><span class="${p.live ? 'acT' : ''}">${p.live ? p.live + ' live' : esc(when(p.last_active_at))}</span>${pcDots(p)}</div>
      <div class="pacts">${pinBtn(p)}${p.root ? `<button class="ib" data-act="project-new" data-dir="${esc(p.root)}" data-name="${esc(p.name)}" data-profile="" title="New session here" aria-label="New session in ${esc(p.name)}">${ic.play}</button>` : ''}<button class="ib" data-act="project-menu" data-key="${esc(p.key)}" data-name="${esc(p.name)}" data-root="${esc(p.root)}" data-pinned="${p.pinned ? 1 : ''}" aria-label="More for ${esc(p.name)}">${ic.dots}</button></div></div>`).join('') || '<p class="empty">No sessions in this range yet.</p>';
  if (pager) pager.innerHTML = sorted.length > 10 ? `<button class="btn sm" data-act="p-set" data-k="page" data-v="${Math.max(0, pj.page - 1)}" ${pj.page === 0 ? 'disabled' : ''} aria-label="Previous page">‹</button>${Array.from({ length: pages }, (_, i) => `<button class="btn sm ${i === pj.page ? 'on' : ''}" data-act="p-set" data-k="page" data-v="${i}" data-testid="ppage-${i}">${i + 1}</button>`).join('')}<button class="btn sm" data-act="p-set" data-k="page" data-v="${Math.min(pages - 1, pj.page + 1)}" ${pj.page === pages - 1 ? 'disabled' : ''} aria-label="Next page">›</button>
      <span class="help">${pj.page * pj.per + 1}–${Math.min(sorted.length, (pj.page + 1) * pj.per)} of ${sorted.length}</span><span class="seg3 mono">${[10, 25, 50].map((n) => `<a class="${pj.per === n ? 'on' : ''}" data-act="p-set" data-k="per" data-v="${n}" data-testid="pper-${n}">${n}</a>`).join('')}</span><span class="help">a page</span>` : '';
}

/** Called by the background refresh: new numbers in place, nothing else moves. */
// an unpin from the sidebar: the Projects page fetches again; a project's header icon flips
document.addEventListener('hm:unpinned', (e) => {
  if (S.route.view === 'projects') { refreshProjects(); return; }
  const b = document.querySelector(`.ib.pin[data-key="${CSS.escape(e.detail)}"]`);
  if (b) { b.classList.remove('on'); b.dataset.on = '1'; b.title = 'Pin to the sidebar'; }
});
export async function refreshProjects() {
  if (S.route.view !== 'projects' || !pj.list) return;
  await fetchProjects().catch(() => {});
  drawTiles(); drawDist(); drawRows();
}

action('p-set', async (el) => {
  const k = el.dataset.k, v = el.dataset.v;
  if (k === 'days') { pj.days = v; pjHash(); await fetchProjects(); pj.page = 0; drawTiles(); drawDist(); drawRows(); return; }
  if (k === 'metric') { pj.metric = v; pjHash(); drawDist(); return; }
  if (k === 'per') { pj.per = +v; pj.page = 0; } else if (k === 'page') pj.page = +v; else pj.sort = v;
  pjHash(); drawRows();
});
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
  if (S.route.view === 'projects') { drawRows(); drawPins(pj.list); return; }
  // elsewhere (a project's Sessions header): the icon flips and the sidebar follows, nothing else moves
  el.classList.toggle('on', on); el.dataset.on = on ? '' : '1'; el.title = on ? 'Pinned to the sidebar: click to unpin' : 'Pin to the sidebar';
  call('projects', { days: 30 }).then(drawPins).catch(() => {});
});
/** Rename a project where its name is: the pencil, a double-click, or the menu. Enter saves, Esc cancels. */
export function renameProject(key) {
  const el = document.querySelector(`[data-name="${CSS.escape(key)}"]`);
  if (!el || el.querySelector('input')) return;
  const old = el.textContent;
  el.innerHTML = `<input value="${esc(old)}" aria-label="Project name" maxlength="80" data-testid="rename-input">`;
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
  { label: 'Reset name', run: () => mutate(() => call('project_alias', { key: el.dataset.key }), 'Reset') },
]));
function projectDialog(key, name, mode) {
  openDialog(`${dlgHead(mode === 'rename' ? 'Rename project' : 'Merge project')}
    <div class="df"><label>Project</label><div class="txt">${esc(name)} <span class="hint mono">${esc(key)}</span></div></div>
    <div class="df"><label for="pj-v">${mode === 'rename' ? 'New name' : 'Merge into key'}</label><div><input class="input" id="pj-v" value="${mode === 'rename' ? esc(name) : ''}">${mode === 'merge' ? '<span class="hint">The key of the project to merge into, as shown under its name</span>' : ''}</div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="pj-ok">Save</button></span></div>`);
  document.getElementById('pj-ok').onclick = async () => {
    const v = document.getElementById('pj-v').value.trim();
    if (await attempt(() => call('project_alias', mode === 'rename' ? { key, name: v } : { key, merge_into: v }), 'Saved')) { closeDialog(); changed(); }
  };
}

// ---------- read a session ----------
// a header, five numbers, and the conversation as a chat with each turn's steps (folded) where they happened;
// Changes and Failed list only those steps. open: the steps unfolded; more: the long runs shown in full;
// follow: stay at the end of the conversation as a live session grows
const rv = { mode: 'all', open: new Set(), more: new Set(), sessionKey: '', follow: false, top: 0 };
const secsOf = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : ms + ' ms');

view('session', async (sheet, params) => {
  sheet.className = 'sheet';
  // a redraw of the shown session (live update, unfolding a step) keeps it on screen until the new one is ready
  if (rv.sessionKey !== params.profile + '/' + params.id || !sheet.querySelector('#chat')) sheet.innerHTML = '<p class="muted">Opening the session…</p>';
  // a session from another computer: its transcript is fetched from the sync folder the first time
  if (params.device && rv.sessionKey !== params.profile + '/' + params.id) toast('Fetching from the folder…');
  const [t, ss, tl] = await Promise.all([call('transcript', { profile: params.profile, id: params.id, device: params.device }), call('session_stats', { profile: params.profile, id: params.id }).catch(() => null), S.cache.tags ? null : call('tag_list').catch(() => [])]);
  if (tl) S.cache.tags = tl;
  if (rv.sessionKey !== params.profile + '/' + params.id) { rv.sessionKey = params.profile + '/' + params.id; rv.open = new Set(); rv.more = new Set(); rv.follow = false; rv.top = 0; rv.fresh = true; }
  const m = t.meta;
  const steps = t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking');
  const changes = steps.filter((st) => st.change), failed = steps.filter((st) => st.status === 'error');
  const win = !S.state || !S.state.os || S.state.os === 'windows';
  const term = !win ? 'Terminal' : S.state.settings.terminal === 'console' ? 'a console window' : 'Windows Terminal';
  const tool = TOOL[m.harness] || m.harness;
  const project = m.project_name || (m.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder';
  // one step: ✓ / ✗ / …, the tool, what it worked on, its time or its change; it unfolds to its output or diff
  const row = (st, k) => {
    const open = rv.open.has(k), bad = st.status === 'error', busy = st.status === 'running' || st.status === 'pending';
    const meta = st.change ? `+${st.change.added} −${st.change.removed}` : bad ? `failed${st.duration_ms ? ' · ' + secsOf(st.duration_ms) : ''}` : st.duration_ms ? secsOf(st.duration_ms) : busy ? 'running now' : '';
    const detail = st.change ? diffHtml(st.change.diff) : `<pre class="sout mono">${esc(st.error || st.result || 'No output.')}</pre>`;
    return `<div class="si ${open ? 'open' : ''}"><button class="sb" data-act="step-open" data-k="${k}" aria-expanded="${open}"><span class="ico ${bad ? 'negT' : busy ? 'warnT' : 'acT'}">${bad ? '✗' : busy ? '…' : '✓'}</span><span class="kind">${esc(st.tool)}</span><span class="what mono" title="${esc(st.input)}">${esc(st.input)}</span><span class="meta mono ${bad ? 'negT' : ''}">${esc(meta)}</span></button>${open ? `<div class="sdet">${detail}</div>` : ''}</div>`;
  };
  // a run of steps; a long one (thousands in a Codex run) shows its first and last until asked for all
  const run = (list, id, head, tail) => !list.length ? '' : `<div class="steps">${rv.more.has(id) || list.length <= head + tail + 2 ? list.map(([st, k]) => row(st, k)).join('')
    : list.slice(0, head).map(([st, k]) => row(st, k)).join('') + `<div class="si more"><a class="acT" data-act="more-steps" data-run="${id}" data-testid="more-steps">Show ${(list.length - head - tail).toLocaleString()} more steps</a></div>` + list.slice(-tail).map(([st, k]) => row(st, k)).join('')}</div>`;
  const files = new Set(changes.map((st) => st.change.path)).size;
  const who = (x) => `<span class="who mono">${esc(tool.toUpperCase())}${x.answer_at ? ' · ' + hm(x.answer_at) : ''}${x.model ? ' · ' + esc(x.model) : ''}</span>`;
  // All: each turn in order, the prompt, then the replies with the steps between them where they happened
  let k = 0;
  const chatAll = t.turns.map((x, ti) => {
    let out = x.prompt ? `<div class="msg you"><span class="who mono">YOU${x.prompt_at ? ' · ' + hm(x.prompt_at) : ''}</span><div class="tx">${esc(x.prompt)}</div></div>` : '';
    const at = x.steps.map((st) => (st.kind === 'thinking' ? -1 : k++));
    const pos = x.answer.map((_, i) => (x.answer_pos && x.answer_pos[i] != null ? x.answer_pos[i] : x.steps.length));
    let from = 0;
    const flush = (to) => { const l = x.steps.slice(from, to).map((st, i) => [st, at[from + i]]).filter(([, j]) => j >= 0); out += run(l, ti + ':' + from, 5, 5); from = to; };
    // replies with no step between them share one bubble
    const groups = [];
    x.answer.forEach((a, i) => { if (!groups.length || groups[groups.length - 1].p !== pos[i]) groups.push({ p: pos[i], a: [] }); groups[groups.length - 1].a.push(a); });
    for (const g of groups) { flush(Math.max(from, g.p)); out += `<div class="msg ai">${who(x)}<div class="tx ans">${g.a.map(md).join('')}</div></div>`; }
    flush(x.steps.length);
    return out;
  }).join('');
  // Changes and Failed: only those steps, as one list
  const picked = steps.map((st, j) => [st, j]).filter(([st]) => (rv.mode === 'changes' ? st.change : st.status === 'error'));
  const body = rv.mode === 'all' ? chatAll : run(picked, 'flat', 30, 10) || `<p class="empty">${rv.mode === 'changes' ? 'This session changed no files.' : 'Nothing failed.'}</p>`;
  sheet.innerHTML = `
    <div class="trow sesh"><div><a href="#sessions" class="crumb acT">← Sessions · ${esc(project)}</a><h1 data-testid="session-title">${esc(m.title || 'Untitled session')}</h1>
        <div class="smeta">${m.open ? `<span class="pill teal mono live">● LIVE${rv.follow ? ' · following' : ''}</span>` : ''}${m.archived ? '<span class="pill arch" data-testid="session-archived">archived</span>' : ''}<span>${esc(acctName(m.profile))} · ${esc(tool)}</span>${tagsHtml(m.tags, 'session', m.profile + '/' + m.id, m.title || 'this session')}${m.model ? `<span class="mono">${esc(m.model)}</span>` : ''}
          ${params.device ? `<span data-testid="session-from">ran on ${esc(m.device_name || 'another computer')}</span>` : m.continued_from ? `<span data-testid="session-from">continued from ${esc(m.continued_from)}</span>` : ''}
          <span>started ${esc(when(m.started_at))} · ${m.prompts} ${m.prompts === 1 ? 'turn' : 'turns'}</span><span class="mono">${fmt(m.tokens)} tokens${ss && ss.cost != null ? ' · $' + ss.cost.toFixed(2) : ''}</span></div></div>
      <div class="acts">${params.device ? '' : `<button class="btn" data-testid="export-md" data-act="export-md" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}">Export as Markdown</button>`}
        ${params.device ? (m.resumable === true ? `<button class="btn primary" data-act="resume-here" data-s="${resumeData(m)}" data-testid="resume-here">Resume here</button>` : `<span class="muted" title="${esc(m.resumable || '')}" data-testid="resume-reason">${esc(reasonShort(m.resumable))}</span>`)
        : m.archived ? '<span class="muted" data-testid="resume-reason">archived: it can be read, not resumed</span>'
        : `<span class="splitb"><button data-act="resume" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}" data-cwd="${esc(m.cwd)}" data-testid="resume">Resume in ${term}</button>${win ? `<button data-act="terminal-menu" aria-label="Choose terminal">${ic.caret}</button>` : ''}</span>`}
        <button class="iconbtn sq" data-act="session-menu" data-path="${esc(m.source_path)}" data-cwd="${esc(m.cwd)}" aria-label="More">${ic.dots}</button></div></div>
    <div class="ovkpis skpis">${[['Tokens', fmt(m.tokens)], ['Read from cache', ss && ss.requests ? Math.round(ss.hit * 100) + '%' : '—'], ['First reply p50', ss && ss.p50 != null ? (ss.p50 / 1000).toFixed(1) + ' s' : '—'],
      ['Failed commands', String(failed.length), failed.length ? 'negT' : ''], ['Files changed', String(files)]]
      .map(([l, v, c]) => `<div class="ovk"><div class="kl"><span>${l}</span></div><strong class="${c || ''}">${v}</strong></div>`).join('')}</div>
    <section class="panel convp" data-testid="conv-panel"><div class="ph"><h2>Conversation <span class="muted mono sm">${t.turns.length} ${t.turns.length === 1 ? 'turn' : 'turns'} · ${steps.length} steps · ${files} changed ${files === 1 ? 'file' : 'files'} · ${failed.length} failed</span></h2>
        <span class="seg3">${[['all', 'All'], ['changes', 'Changes'], ['failed', 'Failed']].map(([v, l]) => `<a class="${rv.mode === v ? 'on' : ''}" data-act="rv-mode" data-v="${v}" data-testid="mode-${v}">${l}</a>`).join('')}</span></div>
      <div class="chat" id="chat" data-testid="chat">${body || '<p class="empty">This session has no messages yet.</p>'}
        <div class="toend" hidden><button class="btn primary sm" data-act="follow-end" data-testid="follow-end">↓ Latest${m.open ? ' · follow live' : ''}</button></div></div></section>`;
  S.cache.transcript = t;
  // a live session opens at its end, following; a redraw keeps the reader's place, or the end while following
  const chat = document.getElementById('chat');
  if (rv.fresh) { rv.fresh = false; rv.follow = m.open; }
  chat.scrollTop = rv.follow ? chat.scrollHeight : rv.top;
  const atEnd = () => chat.scrollHeight - chat.scrollTop - chat.clientHeight < 40;
  const show = () => { chat.querySelector('.toend').hidden = atEnd(); };
  chat.addEventListener('scroll', () => { rv.follow = atEnd(); rv.top = chat.scrollTop; show(); }, { passive: true });
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

// session groups folded shut, by grouping and group
const sFold = new Set();
action('s-fold', (el) => { const k = el.dataset.k; if (sFold.has(k)) sFold.delete(k); else sFold.add(k); render(); });

// ---------- usage ----------

/** Tokens per day, each day's bar split by account. */
export function stacked(days, series, w = 900) {
  const h = 200, padB = 22, n = days.length, gap = Math.max(4, Math.min(14, Math.round((w / n) * 0.18))), bw = (w - gap * (n - 1)) / n;
  const tot = days.map((_, i) => series.reduce((a, x) => a + x.vals[i], 0)), max = Math.max(1, ...tot);
  let svg = `<svg width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" style="display:block;max-width:100%" role="img" aria-label="Tokens per day, by account" data-testid="usage-chart">`;
  days.forEach((d, i) => {
    let y = h - padB;
    const x = i * (bw + gap);
    series.forEach((sr) => {
      const v = sr.vals[i]; if (!v) return;
      const bh = Math.max(1, (v / max) * (h - padB - 8));
      y -= bh;
      svg += `<rect x="${x.toFixed(1)}" y="${y.toFixed(1)}" width="${bw.toFixed(1)}" height="${bh.toFixed(1)}" style="fill:${sr.color}"><title>${esc(sr.name)} · ${esc(d)}: ${fmt(v)} tokens</title></rect>`;
      y -= 1;
    });
    const last = i === n - 1, step = Math.max(1, Math.round(n / 3));
    if (i === 0 || last || i % step === 0) svg += `<text x="${last ? x + bw : x}" y="${h - 6}" text-anchor="${last ? 'end' : 'start'}" font-size="11" style="fill:var(--m);font-family:var(--mono)">${last ? 'today' : esc(new Date(d + 'T12:00').toLocaleDateString([], { day: 'numeric', month: 'short' }))}</text>`;
  });
  return svg + '</svg>';
}

// Usage by account lives on Accounts now; the old address still works
view('usage', async () => { go('accounts'); });

// ---------- routing ----------
view('routing', async (sheet) => {
  sheet.className = 'sheet';
  const r = await call('routing');
  const st = await call('state');
  S.state = st;
  const now = r.now, x = r.stats;
  const paused = (id) => r.breakers[id] && r.breakers[id].state === 'open';
  const nPaused = r.providers.filter((p) => paused(p.id)).length;
  const users = (route) => r.routed_profiles.filter((p) => p.route === route).map((p) => acctName(p.name));
  const pname = (id) => (r.providers.find((p) => p.id === id) || { name: id }).name;
  const secs = (ms) => (ms == null ? '—' : (ms / 1000).toFixed(1) + ' s');
  // a route's providers as a chain: serving, paused (with when it is tried again) or standing by
  const chain = (x2) => x2.providers.map((id, k) => {
    const b = r.breakers[id] || {}, isDefault = x2.mode === 'aggregate' && (x2.default_provider || x2.providers[0]) === id;
    const state = paused(id) ? `paused${b.retry_at && b.retry_at > now ? ' · retry ' + dur(b.retry_at - now) : ''}` : x2.mode === 'aggregate' ? (isDefault ? 'default' : 'hm-' + id + '--… models') : k === 0 || paused(x2.providers[k - 1]) ? 'serving' : 'standby';
    const tone = paused(id) ? 'bad' : state === 'serving' || state === 'default' ? 'ok' : '';
    return `${k ? '<span class="arr" aria-hidden="true">→</span>' : ''}<span class="pchip ${tone}"><b>${esc(pname(id))}</b><small class="mono">${esc(state)}</small></span>`;
  }).join('');
  sheet.innerHTML = `
    <div class="trow"><div><div class="crumb">API keys &amp; routing</div><h1>${r.running ? `The router is up on :${r.port}: ${x.requests.toLocaleString()} ${x.requests === 1 ? 'request' : 'requests'} in 24 h${nPaused ? `; <span class="w">${nPaused} ${nPaused === 1 ? 'provider is' : 'providers are'} paused</span>` : x.errors ? `, ${x.errors} ${x.errors === 1 ? 'error' : 'errors'}` : ''}.` : 'The local router is stopped.'}</h1><p class="find">Keys live only in Windows Credential Manager. The local router sends routed accounts to providers, moves to the next one when a provider fails, and pauses one that keeps failing. Subscription logins never pass through it.</p></div>
      <div class="acts"><button class="btn" data-act="provider-add" data-testid="provider-add">Add provider</button><button class="btn primary" data-act="route-add" data-testid="route-add" ${r.providers.length ? '' : 'disabled'}>Add route</button></div></div>
    <section class="panel rcard"><div class="rl"><i class="sdot ${r.running ? 'live' : ''}"></i><div><h2 data-testid="router-state">Local router · ${r.running ? 'running' : 'stopped'}</h2>
        <div class="mono muted sm">127.0.0.1:${r.port} · token required · ${r.routes.length} ${r.routes.length === 1 ? 'route' : 'routes'} · ${r.routed_profiles.length} ${r.routed_profiles.length === 1 ? 'account uses' : 'accounts use'} it</div>
        <div style="margin-top:14px">${r.running ? '<button class="btn" data-act="router-stop">Stop router</button>' : '<button class="btn" data-act="router-start" data-testid="router-start">Start router</button>'}</div></div></div>
      <div class="rstats"><div><span>Requests · 24 h</span><b class="mono">${x.requests.toLocaleString()}</b></div><div><span>Failovers</span><b class="mono ${x.failovers ? 'warnT' : ''}">${x.failovers}</b></div>
        <div><span>Errors</span><b class="mono ${x.errors ? 'negT' : ''}">${x.errors}</b></div><div><span>Paused providers</span><b class="mono ${nPaused ? 'negT' : ''}">${nPaused}</b></div></div></section>
    <h2 class="sech">Routes</h2>
    ${r.routes.length ? `<div class="rgrid" data-testid="routes">${r.routes.map((x2) => `<section class="panel route"><div class="ph"><h2 class="mono">${esc(x2.id)}</h2><span class="muted sm">${x2.mode === 'aggregate' ? 'aggregate' : 'failover'} · used by ${esc(users(x2.id).join(', ') || 'no account yet')}</span></div>
        <div class="pchain">${chain(x2)}</div>
        <p class="hint">${x2.mode === 'aggregate' ? 'One model list from all its providers, converted between the Anthropic and OpenAI formats.' : 'Tries each provider in order; one that fails five times in a row is skipped for a while.'}</p>
        <div class="racts"><button class="btn danger sm" data-act="route-remove" data-id="${esc(x2.id)}">Remove</button></div></section>`).join('')}</div>`
      : '<p class="empty">No routes yet. Add a provider, then a route that uses it.</p>'}
    <section class="panel"><div class="ph"><h2>Providers</h2></div>
    ${r.providers.length ? `<table class="grid" data-testid="providers"><thead><tr><th>Provider</th><th>Format</th><th>Base URL</th><th>Key</th><th>Breaker</th><th class="num">Requests · 24 h</th><th class="num">First chunk p95</th><th></th></tr></thead><tbody>${r.providers.map((p) => { const b = r.breakers[p.id] || {}, ps = x.providers[p.id] || {}; return `<tr><td><b>${esc(p.name)}</b> <span class="muted mono">${p.name === p.id ? '' : esc(p.id)}</span></td><td class="mono">${p.format}</td><td class="mono">${esc(p.base_url)}</td><td>${r.keys[p.id] ? 'saved' : '<span class="err">missing</span>'}</td><td>${paused(p.id) ? `<span class="pill neg">open · ${b.failures} failures</span>` : '<span class="pill teal">closed</span>'}</td><td class="num">${(ps.requests || 0).toLocaleString()}</td><td class="num">${secs(ps.p95_first)}</td><td class="num"><button class="btn danger sm" data-act="provider-remove" data-id="${esc(p.id)}">Remove</button></td></tr>`; }).join('')}</tbody></table>` : '<p class="empty">No providers yet. Add OpenRouter, DeepSeek or any Anthropic- or OpenAI-compatible endpoint.</p>'}</section>
    <section class="panel"><div class="ph"><h2>Recent requests through the router</h2></div>
    ${r.log.length ? `<table class="grid"><thead><tr><th>Time</th><th>Account</th><th>Route → provider</th><th>Model</th><th>Status</th><th class="num">First chunk</th><th class="num">Time</th><th class="num">Tokens</th></tr></thead><tbody>${r.log.map((l) => `<tr><td class="mono">${esc(hm(l.ts))}</td><td>${esc(acctName(l.profile))}</td><td class="mono">${esc(l.route)} → ${esc(pname(l.provider))}</td><td class="mono">${esc(l.model)}</td><td class="mono ${l.status === 200 ? 'acT' : 'negT'}">${l.status || 'failed'}</td><td class="num">${secs(l.first_ms)}</td><td class="num">${secs(l.ms)}</td><td class="num">${l.input + l.output ? fmt(l.input + l.output) : '—'}</td></tr>`).join('')}</tbody></table>` : '<p class="empty">No requests yet.</p>'}</section>`;
});
action('router-start', () => mutate(() => call('router_start'), 'Router started'));
action('router-stop', () => mutate(() => call('router_stop'), 'Router stopped'));
action('provider-add', () => {
  openDialog(`${dlgHead('Add provider')}
    <div class="df"><label for="pv-id">Id</label><div><input class="input" id="pv-id" placeholder="openrouter" data-testid="pv-id"></div></div>
    <div class="df"><label for="pv-name">Name</label><div><input class="input" id="pv-name" placeholder="OpenRouter"></div></div>
    <div class="df"><label for="pv-format">Format</label><div><select class="input" id="pv-format" data-testid="pv-format"><option value="anthropic">Anthropic Messages</option><option value="openai">OpenAI Chat Completions</option></select></div></div>
    <div class="df"><label for="pv-url">Base URL</label><div><input class="input" id="pv-url" placeholder="https://openrouter.ai/api/v1" data-testid="pv-url"><span class="hint">Anthropic: the root (…/v1/messages is added). OpenAI: up to /v1 (…/chat/completions is added).</span></div></div>
    <div class="df"><label for="pv-models">Models</label><div><input class="input" id="pv-models" placeholder="deepseek/deepseek-v4, z-ai/glm-5" data-testid="pv-models"><span class="hint">Comma separated. The first is used when a request names a model this provider lacks.</span></div></div>
    <div class="df"><label for="pv-key">Key</label><div><input class="input" type="password" id="pv-key" data-testid="pv-key"><span class="hint">Kept in Windows Credential Manager</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="pv-ok" data-testid="pv-save">Add provider</button></span></div>`);
  document.getElementById('pv-ok').onclick = async () => {
    const g = (id) => document.getElementById(id).value.trim();
    if (await attempt(() => call('provider_add', { id: g('pv-id'), name: g('pv-name'), format: g('pv-format'), base_url: g('pv-url'), models: g('pv-models'), key: g('pv-key') }), 'Provider added')) { closeDialog(); changed(); }
  };
});
action('provider-remove', async (el) => { if (await confirmDialog('Remove provider', `Remove <b>${esc(el.dataset.id)}</b> and its saved key?`, 'Remove')) mutate(() => call('provider_remove', { id: el.dataset.id }), 'Removed'); });
action('route-add', async () => {
  const r = await call('routing');
  openDialog(`${dlgHead('Add route')}
    <div class="df"><label for="rt-id">Id</label><div><input class="input" id="rt-id" placeholder="cheap" data-testid="rt-id"></div></div>
    <div class="df"><label for="rt-mode">Mode</label><div><select class="input" id="rt-mode" data-testid="rt-mode"><option value="failover">Failover: try providers in order</option><option value="aggregate">One model list across providers</option></select></div></div>
    <div class="df"><label>Providers</label><div>${r.providers.map((p, i) => `<label class="switch" style="display:flex;margin:4px 0"><input type="checkbox" value="${esc(p.id)}" class="rt-p" data-testid="rt-p-${esc(p.id)}" ${i === 0 ? 'checked' : ''}> ${esc(p.name)} <span class="muted mono">${esc(p.id)}</span></label>`).join('')}<span class="hint">Checked in this order; failover tries them top to bottom.</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="rt-ok" data-testid="rt-save">Add route</button></span></div>`);
  document.getElementById('rt-ok').onclick = async () => {
    const providers = [...document.querySelectorAll('.rt-p:checked')].map((c) => c.value);
    if (await attempt(() => call('route_add', { id: document.getElementById('rt-id').value.trim(), mode: document.getElementById('rt-mode').value, providers }), 'Route added')) { closeDialog(); changed(); }
  };
});
action('route-remove', async (el) => { if (await confirmDialog('Remove route', `Remove route <b>${esc(el.dataset.id)}</b>?`, 'Remove')) mutate(() => call('route_remove', { id: el.dataset.id }), 'Removed'); });

// ---------- MCP servers and skills ----------
export function syncReport(rep, item) {
  if (!rep) return;
  if (rep.error) return toast('Saved, but writing into the tools failed: ' + rep.error, true);
  // entries read "<folder>: <name>"; with an item, only its own
  const mine = (l) => (l || []).filter((x) => !item || x.endsWith(': ' + item) || x.includes('skill ' + item + ' '));
  const n = mine(rep.written).length, c = mine(rep.conflicts), s = mine(rep.skipped);
  const copies = c.filter((x) => /\\\.claude-[^\\]+\\skills: /.test(x)).length;
  // a skill's own folder in a tool (not a link): Replace moves it to .hrm-backup there and links the store's copy
  const skills = c.filter((x) => !/\.json: /.test(x)), names = [...new Set(skills.map((x) => x.replace(/^.*: /, '')))];
  if (c.length) {
    const msg = `${item || 'Some items'} not linked into ${c.length} ${c.length === 1 ? 'folder' : 'folders'} that already ${c.length === 1 ? 'has' : 'have'} its own copy, so ${c.length === 1 ? 'it was' : 'they were'} left alone: ${c[0].replace(/: [^:]*$/, '')}${c.length > 1 ? ' and more' : ''}.${copies ? ' Settings > Link copies to the main profile fixes the Claude accounts\' old copies.' : ''}`;
    toast(msg, true);
    if (skills.length) notice(msg + ' Replace moves each copy into .hrm-backup in that folder and links the shared one instead.', { act: 'skill-takeover', label: `Replace ${skills.length === 1 ? 'the copy' : 'the ' + skills.length + ' copies'} with links`, data: { names: JSON.stringify(names) } });
  }
  else if (s.length) toast(`Skipped: ${s[0]}`, true);
  else toast(n ? `Written to ${n} ${n === 1 ? 'place' : 'places'}` : 'Saved');
}

/** A tool column's header tick and count, from the rows shown: all, some (shown indeterminate) or none. */
export function headState(table) {
  if (!table || !table.tHead) return;
  for (const th of table.tHead.rows[0].cells) {
    const head = th.querySelector('input[type=checkbox]');
    if (!head) continue;
    const boxes = [...table.querySelectorAll(`tbody tr:not([hidden]) input[data-h="${head.dataset.h}"]`)], on = boxes.filter((b) => b.checked).length;
    head.checked = boxes.length > 0 && on === boxes.length;
    head.indeterminate = on > 0 && on < boxes.length;
    const small = th.querySelector('small');
    if (small) { small.textContent = boxes.length ? String(on) : ''; small.title = `${on} of ${boxes.length} shown`; }
  }
}
action('toggle-target', async (el) => {
  const rep = await attempt(() => call('target', { kind: el.dataset.kind, item: el.dataset.item, harness: el.dataset.h, on: el.checked }));
  if (rep === undefined) { el.checked = !el.checked; return; }
  syncReport(rep, el.dataset.item);
  // the column's count under its tool icon, and the row's Off tag, follow the ticks
  // (the skills list has no header row: its tools are chips)
  const table = el.closest('table'), small = table && table.tHead && table.tHead.rows[0].cells[el.closest('td').cellIndex]?.querySelector('small');
  if (small) small.textContent = table.querySelectorAll(`input[data-h="${el.dataset.h}"]:checked`).length;
  if (table && table.classList.contains('sklist')) headState(table);
  const th = table && table.tHead && table.tHead.rows[0].cells[el.closest('td').cellIndex];
  if (th && th.dataset.label) th.title = `${th.dataset.label}: ${table.querySelectorAll(`input[data-h="${el.dataset.h}"]:checked`).length} servers`;
  const row = el.closest('tr'), off = row.querySelector('.tag.off'), any = row.querySelector('input[type=checkbox]:checked');
  if (off && any) off.remove();
  else if (!off && !any && row.querySelector('.mn')) row.querySelector('.mn').insertAdjacentHTML('beforeend', '<span class="tag off" title="Not written to any tool">Off</span>');
});
action('skill-takeover', async (el) => {
  const names = JSON.parse(el.dataset.names || '[]');
  if (!(await confirmDialog('Replace the tools\' copies with links?', `The ${names.length === 1 ? 'copy' : 'copies'} of <b>${esc(names.join(', '))}</b> in the tools' own skill folders ${names.length === 1 ? 'moves' : 'move'} into <span class="mono">.hrm-backup</span> beside ${names.length === 1 ? 'it' : 'them'}, and the shared ${names.length === 1 ? 'copy is' : 'copies are'} linked there instead. Nothing is deleted.`, 'Replace', false))) return;
  document.getElementById('notice').className = 'off';
  const r = await mutate(() => call('skill_takeover', { names }));
  if (!r) return;
  toast(r.moved.length ? `Replaced ${r.moved.length} ${r.moved.length === 1 ? 'copy' : 'copies'}; the originals are in .hrm-backup` : 'Nothing to replace: no copy stood where a link should be');
  syncReport(r.sync);
});
action('item-remove', async (el) => {
  if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from every tool Harness Manager wrote it into?`, 'Remove')) syncReport(await mutate(() => call('item_remove', { kind: el.dataset.kind, name: el.dataset.name })));
});
action('import-from', async (el) => {
  const hs = shownTools(S.state || await call('state')).filter((h) => (el.dataset.kind === 'mcp' ? h.caps.mcp : h.caps.skills));
  openDialog(`${dlgHead(el.dataset.kind === 'mcp' ? 'Import MCP servers' : 'Import skills')}
    <div class="df"><label for="im-h">From</label><div><select class="input" id="im-h">${hs.map((h) => `<option value="${h.id}">${esc(h.label)} (main profile)</option>`).join('')}</select>
    <span class="hint">${el.dataset.kind === 'mcp' ? 'Its servers become shared and stay enabled for that tool.' : 'Its skill folders move to the shared store and are linked back.'}</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="im-ok">Import</button></span></div>`);
  document.getElementById('im-ok').onclick = async () => {
    const r = await attempt(() => call(el.dataset.kind === 'mcp' ? 'mcp_import' : 'skill_import', { harness: document.getElementById('im-h').value }));
    if (r) {
      const added = Array.isArray(r) ? r : r.added, kept = r.kept || [];
      closeDialog();
      toast(`Imported ${added.length}: ${added.join(', ') || 'nothing new'}${kept.length ? '. Kept your own ' + kept.join(', ') + ': a different one is installed' : ''}`);
      changed();
    }
  };
});
action('skill-install', () => {
  openDialog(`${dlgHead('Install skill')}
    <div class="df"><label for="sk-src">Source</label><div style="display:flex;gap:8px"><input class="input" id="sk-src" placeholder="https://github.com/owner/repo/tree/main/skills/pdf or a folder" data-testid="sk-src"><button class="btn outline" id="sk-pick">Folder…</button></div></div>
    <div class="df"><label for="sk-name">Name</label><div><input class="input" id="sk-name" placeholder="Optional; the folder name otherwise"></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="sk-ok" data-testid="sk-save">Install</button></span></div>`);
  document.getElementById('sk-pick').onclick = async () => { const p = await pickFolder(); if (p) document.getElementById('sk-src').value = p; };
  document.getElementById('sk-ok').onclick = async () => {
    const r = await attempt(() => call('skill_install', { source: document.getElementById('sk-src').value.trim(), name: document.getElementById('sk-name').value.trim() }));
    if (r) { closeDialog(); toast(`Installed ${r.name}; tick the tools it should be in`); syncReport(r.sync, r.name); changed(); }
  };
});
// ---------- prompts ----------
let promptTool = 'claude', promptSel = null;
view('prompts', async (sheet) => {
  sheet.className = 'sheet';
  const d = await call('items');
  const hs = shownTools(S.state || await call('state')).filter((h) => h.caps.prompts);
  const list = d.prompts.filter((p) => p.harness === promptTool);
  const live = await call('prompt_live', { harness: promptTool }).catch(() => null);
  const sel = list.find((p) => p.id === promptSel) || null;
  const FILE = { claude: 'CLAUDE.md', codex: 'AGENTS.md', opencode: 'AGENTS.md', agy: 'GEMINI.md', pi: 'AGENTS.md', omp: 'AGENTS.md' };
  const tool = (hs.find((h) => h.id === promptTool) || { label: promptTool }).label;
  const first = (b) => (b || '').split('\n').map((l) => l.replace(/^#+\s*/, '').trim()).find(Boolean) || 'Empty';
  const where = live ? `Writes to <span class="mono">${esc(live.path)}</span> · every ${esc(tool)} account follows it` : `No profile of ${esc(tool)} yet`;
  sheet.innerHTML = `<div class="trow"><div><div class="crumb">System prompts</div><h1>The instructions each tool reads when it starts.</h1><p class="find">CLAUDE.md for Claude Code, AGENTS.md for Codex and OpenCode, GEMINI.md for Antigravity. Keep several here and choose which one is in the file; what the file held before is saved first, so nothing is lost.</p></div>
      <div class="acts"><button class="btn primary" data-act="prompt-new" data-testid="prompt-new">New prompt</button></div></div>
    <div class="tabs2">${hs.map((h) => `<a class="${h.id === promptTool ? 'on' : ''}" data-act="prompt-tool" data-v="${h.id}" title="${esc(h.label)} reads ${FILE[h.id] || 'its prompt file'}">${toolIcon(h.id, 13)} ${esc(h.label)}</a>`).join('')}</div>
    <div class="pgrid"><div class="plist">
      <a class="${!sel ? 'on' : ''}" data-act="prompt-sel" data-id=""><span><b>The file now</b><small>${esc(live ? first(live.body) : 'not there yet')}</small></span><span class="pill mono">live</span></a>
      ${list.map((p) => `<a class="${sel && sel.id === p.id ? 'on' : ''}" data-act="prompt-sel" data-id="${esc(p.id)}" data-testid="prompt-${esc(p.id)}"><span><b>${esc(p.title)}</b><small>${esc(first(p.body))}</small></span>${p.enabled ? '<span class="pill teal mono">enabled</span>' : ''}</a>`).join('')}</div>
    <div class="peditor">${sel ? `<div class="ph"><input class="input ptitle" id="pr-title" value="${esc(sel.title)}" aria-label="Prompt name">${sel.enabled ? `<span class="pill teal mono">enabled in ${esc(tool)}</span>` : ''}</div>
      <p class="pwhere">${where}</p>
      <textarea class="input" id="pr-body" aria-label="Prompt text">${esc(sel.body)}</textarea>
      <div class="pfoot"><span class="muted sm">${sel.enabled ? 'This prompt is in the file now.' : 'Not in the file yet.'}</span><span class="acts">
        <button class="btn danger" data-act="prompt-delete" data-id="${esc(sel.id)}">Delete</button>
        <button class="btn" data-act="prompt-sel" data-id="">Show the live file</button>
        ${sel.enabled ? '' : `<button class="btn" data-act="prompt-enable" data-id="${esc(sel.id)}" data-testid="prompt-enable">Use this prompt</button>`}
        <button class="btn primary" data-act="prompt-save" data-id="${esc(sel.id)}">Save</button></span></div>`
      : `<div class="ph"><h2>The file now</h2><span class="pill mono">live</span></div><p class="pwhere">${where}</p>
        <div class="code" style="white-space:pre-wrap;min-height:360px">${esc(live ? live.body || '(the file is empty)' : 'No profile of this tool yet.')}</div>`}</div></div>`;
});
action('prompt-tool', (el) => { promptTool = el.dataset.v; promptSel = null; render(); });
action('prompt-sel', (el) => { promptSel = el.dataset.id || null; render(); });
action('prompt-new', async () => { const id = await attempt(() => call('prompt_add', { harness: promptTool, title: 'New prompt', body: '' })); if (id) promptSel = id; changed(); });
action('prompt-save', (el) => mutate(() => call('prompt_save', { id: el.dataset.id, title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value }), 'Saved'));
action('prompt-enable', async (el) => { if (!(await attempt(() => call('prompt_save', { id: el.dataset.id, title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value })))) return; mutate(() => call('prompt_enable', { id: el.dataset.id }), 'Written to the prompt file'); });
action('prompt-delete', async (el) => { if (await confirmDialog('Delete prompt', 'Delete this prompt from the library? The prompt file is not changed.', 'Delete')) { if (await attempt(() => call('prompt_delete', { id: el.dataset.id }))) promptSel = null; changed(); } });

// ---------- settings ----------
/** An ⓘ that explains a setting on hover or keyboard focus. */
const info = (id, html) => `<span class="tip"><button type="button" class="tipb" aria-label="About this setting" aria-describedby="tip-${id}" data-testid="tip-${id}">ⓘ</button><span class="bubble" role="tooltip" id="tip-${id}">${html}</span></span>`;

// a switch that saves as soon as it is flipped
const sw = (key, on, label, testid) => `<label class="switch"><input type="checkbox" data-act="set-now" data-key="${key}" ${on ? 'checked' : ''} data-testid="${testid}"> ${label}</label>`;
const seg = (key, cur, opts, testid) => `<span class="seg3" data-testid="${testid}">${opts.map(([v, l]) => `<a class="${cur === v ? 'on' : ''}" data-act="set-now" data-key="${key}" data-v="${v}">${l}</a>`).join('')}</span>`;
const NEW = '<span class="new">NEW</span>';
const row = (title, tip, text, ctl) => `<div class="frow"><div><b>${title}</b>${tip || ''}<p>${text}</p></div><div class="ctl">${ctl}</div></div>`;

view('settings', async (sheet) => {
  sheet.className = 'sheet';
  const st = await call('state');
  S.state = st;
  applyTheme(st);
  const s = st.settings;
  // the sign-in entry and the choice of terminal are Windows things; macOS opens Terminal
  const win = !st.os || st.os === 'windows';
  sheet.innerHTML = `<div class="crumb">Settings</div><h1>Saved as you change them, for this computer.</h1><p class="find">Keys and logins are never in these files or in backups.</p>
    <h5 class="sh5">Appearance</h5>
    ${row('Theme', info('brand', '<b>Harness (the default):</b> porcelain and night, Inter.<br><b>VektraGrid:</b> ivory and deep green, Garamond headings; deep teal leads, plum marks the router, sand gold is the warning.'), 'The look of this window; light and dark follow the choice below.', seg('brand', s.brand || 'hm', [['hm', 'Harness'], ['vg', 'VektraGrid']], 'st-brand'))}
    ${row('Light and dark', info('theme', '<b>System (the default):</b> follows the light or dark setting of Windows or macOS.<br><b>Light</b> or <b>Dark</b>: always that.'), 'The colours of this window.', seg('theme', s.theme, [['system', 'System'], ['light', 'Light'], ['dark', 'Dark']], 'st-theme'))}
    ${row('Limits shown as', info('quota', '<b>Left (the default):</b> how much of each limit you still have, as the tools show it.<br><b>Used:</b> how much has gone.'), 'What is left of each limit, or what is used.', seg('quota_display', s.quota_display, [['left', 'Left'], ['used', 'Used']], 'st-quota'))}
    ${row('Moving routes' + NEW, info('motion', 'Pulses along the live routes, as fast as their traffic. Off, or with reduced motion in Windows or macOS, the routes are a still picture.<br><b>Default: on.</b>'), "The live particles on Overview. Off, or the system's reduced motion, keeps them still.", sw('moving_routes', s.moving_routes, 'Animate the routes', 'st-motion'))}
    <h5 class="sh5">${win ? 'Window and terminal' : 'Window'}</h5>
    ${row('Close to the tray', info('tray', '<b>On (the default):</b> the window\'s X hides Harness Manager to the tray; usage and limits keep refreshing, and the tray icon opens it again. Quit from the tray menu.<br><b>Off:</b> the X quits.'), 'The window hides and the app keeps refreshing until you choose Quit.', sw('close_to_tray', s.close_to_tray, 'Keep running in the tray', 'st-tray'))}
    ${win ? row('Launch at sign-in', info('auto', 'Starts Harness Manager in the tray when you sign in to Windows, so usage and limits are current when you open it.<br><b>Default: off.</b>'), 'Starts hidden in the tray.', `<label class="switch"><input type="checkbox" data-act="autostart" ${st.autostart ? 'checked' : ''} data-testid="st-autostart"> Start with Windows</label>`) : ''}
    ${win ? row('Terminal', info('term', '<b>Windows Terminal</b> (the default): Start and Resume open a new Windows Terminal window running the account.<br><b>Console window</b>: the classic console, for computers without Windows Terminal or if you prefer it.'), 'Where Start and Resume open a console.',
      seg('terminal', s.terminal, [['wt', 'Windows Terminal'], ['console', 'Console']], 'st-term')) : ''}
    <h5 class="sh5">Updates</h5>
    ${row('Harness Manager', '', selfUpd && selfUpd.version ? `Version <b>${esc(selfUpd.version)}</b> is available; you have ${esc(st.version)}.${selfUpd.notes ? ' ' + esc(selfUpd.notes) : ''}` : `Version ${esc(st.version)}. New versions are looked for at launch and every hour.`,
      selfUpd && selfUpd.version ? '<button class="btn primary" data-act="self-install" data-testid="self-install">Update and restart</button>' : '<button class="btn outline" data-act="self-check" data-testid="self-check">Check for updates</button>')}
    ${row('New versions of your tools', info('upd', 'Asks each vendor for the newest Claude Code, Codex, OpenCode and Antigravity CLI when Harness Manager starts; Apps in the sidebar shows how many have one.<br><b>Default: off.</b> Apps &gt; Check for updates looks at any time.'), 'Look for newer versions of the tools when the app starts.', sw('check_updates_at_launch', s.check_updates_at_launch, 'Check at launch', 'st-updcheck'))}
    <h5 class="sh5">Usage and limits</h5>
    ${row('Refresh', info('refresh', 'How often, in minutes, Harness Manager reads new usage from the tools\' logs and the cached limits, in the background.<br><b>Default: 5.</b> A smaller number updates sooner but reads the logs more often. <b>Sync now</b> on Home reads them at once whatever this is.'), 'How often usage and limits are read in the background.', seg('refresh_minutes', String(s.refresh_minutes), [['1', '1 min'], ['5', '5 min'], ['15', '15 min']], 'st-refresh'))}
    ${row('Live Claude limits', info('live', '<b>Off (the default):</b> limits are what Claude Code last saved, so they are as fresh as the account\'s last run; a window whose reset time has passed shows as empty.<br><b>On:</b> when those numbers are more than 10 minutes old, Harness Manager asks Anthropic for current ones with the login Claude Code saved, while it is valid. It never refreshes or copies that login.'), 'Off: limits are what Claude Code last wrote. On: Harness Manager also asks Anthropic, with the login Claude Code saved on this computer while it is valid. It never refreshes or copies that login.', `<label class="switch"><input type="checkbox" id="st-live" data-act="live-now" ${st.live_quota ? 'checked' : ''} data-testid="st-live"> Ask Anthropic for current limits</label>`)}
    <h5 class="sh5">Connections</h5>
    ${row('Router port', info('port', 'The port of the local router that routed accounts talk to. It listens on 127.0.0.1 only, so nothing outside this computer can reach it.<br><b>Default: 15731.</b> Change it only if another program uses that port; routed accounts use the new port the next time they start.'), 'The router listens on 127.0.0.1 only.', `<input class="input mono" type="number" id="st-port" value="${s.router_port}" style="max-width:110px" aria-label="Router port"><button class="btn outline" data-act="set-field" data-key="router_port" data-from="st-port" data-testid="st-port-save">Change</button>`)}
    ${row('Account list', info('reg', 'Where profiles.json lives: your accounts, providers, routes, MCP servers, skills and prompts. No keys or logins are ever stored in it.<br><b>Empty (the default):</b> it stays in this computer\'s Harness Manager folder (%APPDATA%\\HarnessManager). Enter a path in a OneDrive folder to share the same accounts with your other computer; each computer keeps its own keys and logins.'), 'Profiles, providers, routes, MCP servers, skills and prompts. No keys or logins are stored in it.', `<input class="input mono" id="st-reg" value="${esc(s.registry_path)}" placeholder="${esc(st.registry_path)}" aria-label="Account list path" style="min-width:300px"><button class="btn outline" data-act="set-field" data-key="registry_path" data-from="st-reg" data-testid="st-reg-save">Move…</button>`)}
    <h5 class="sh5">Diagnostic log</h5>
    ${row('Keep a log', info('log', 'What Harness Manager did and what went wrong (a usage file it could not read, a failed backup), with the time. It holds no keys or logins.<br><b>Default: on, Warnings.</b> Information also records backups, restores and the like.'), 'Kept in the data folder; nothing leaves this computer.', sw('log_enabled', s.log_enabled, 'Keep a log', 'st-log'))}
    ${row('Level', '', 'How much it records.', seg('log_level', s.log_level, [['error', 'Errors'], ['warn', 'Warnings'], ['info', 'Everything']], 'st-loglevel'))}
    ${row('Log file', '', 'Kept in the data folder; it holds no keys or logins.', `<button class="btn outline" data-act="log-open" data-testid="log-open">Open log</button>`)}
    <h5 class="sh5">Maintenance</h5>
    ${row('Find tools again', '', 'Adds a main profile for Claude Code, Codex, OpenCode or Antigravity when found.', '<button class="btn outline" data-act="detect">Find tools</button>')}
    ${row('Old launcher', '', 'Accounts and keys from <span class="mono">claude-accounts.json</span>. Keys can only be read on the computer that saved them.', '<button class="btn outline" data-act="import-ps">Import</button>')}
    ${row('Tours', '', 'The walkthrough each page shows the first time you open it. The ? at the top replays the tour of the page you are on.', '<button class="btn outline" data-act="guides-reset" data-testid="guides-reset">Show the tours again</button>')}
    <h5 class="sh5">Computers</h5>
    <div id="st-computers" data-testid="computers"><p class="muted">Reading computers…</p></div>
    <h5 class="sh5">Backups and archive</h5>
    <div id="st-backups" data-testid="backups"><p class="muted">Reading backups…</p></div>`;
  // every section a card that folds; open by default, and its fields stay on the page while folded
  const [head, ...parts] = sheet.innerHTML.split('<h5 class="sh5">');
  sheet.innerHTML = head + parts.map((part) => {
    const i = part.indexOf('</h5>'), title = part.slice(0, i), id = title.toLowerCase().replace(/[^a-z]+/g, '-'), open = !setFold.has(id);
    const n = (part.match(/class="frow"/g) || []).length;
    return `<section class="panel sset ${open ? '' : 'shut'}" data-testid="sec-${id}"><h2 class="sh5"><button class="fold" data-act="set-fold" data-v="${id}" aria-expanded="${open}">${title}<span class="cnt">${n ? `${n} ${n === 1 ? 'setting' : 'settings'}` : ''}<span class="chev" aria-hidden="true">${open ? '▾' : '▸'}</span></span></button></h2><div class="sbody">${part.slice(i + 5)}</div></section>`;
  }).join('');
  drawComputers(s, st);
  drawBackups(s);
});
const setFold = new Set();
action('set-fold', (el) => {
  const id = el.dataset.v, sec = el.closest('.sset');
  if (setFold.has(id)) setFold.delete(id); else setFold.add(id);
  sec.classList.toggle('shut', setFold.has(id));
  el.setAttribute('aria-expanded', String(!setFold.has(id)));
  el.querySelector('.chev').textContent = setFold.has(id) ? '▸' : '▾';
});

// ---------- Settings > Computers ----------
// this computer's name, how it syncs (a shared folder, or a hub you host), what travels, and a card per computer
const ago = (t) => (Date.now() - t < 60000 ? 'just now' : dur(Date.now() - t) + ' ago');
async function drawComputers(s, st) {
  const c = await computersStatus().catch(() => null);
  const el = document.getElementById('st-computers');
  if (!c || !el) return;
  // phase 3: what this computer has carried, for the Carry transcripts popover
  const ts = syncOn(c) ? await call('transcripts_status').catch(() => null) : null;
  const on = syncOn(c), now = Date.now(), n = computers(c).length;
  // phase 5: the hub this computer joined, if any (computers_status.hub)
  const hub = c.hub || {}, hubOn = on && c.mode === 'hub', hubN = hub.devices || 0;
  const card = (d) => {
    const me = d === c.this, seq = d.last_seq || 0, done = me ? (d.published_seq ?? seq) : (d.applied_seq || 0), pct = seq ? Math.min(100, Math.round((done / seq) * 100)) : 100;
    const stale = !me && !heardOn(d, now);
    const heard = me ? 'writing now' : d.connected ? 'connected' : d.last_seen ? (now - d.last_seen < 90000 ? 'writing now' : 'heard ' + ago(d.last_seen)) : 'not heard from yet';
    const facts = [d.projects != null ? `${d.projects} ${d.projects === 1 ? 'project' : 'projects'} here` : '', d.accounts != null ? `${d.accounts} ${d.accounts === 1 ? 'account' : 'accounts'}` : '', (d.running || []).length ? `${d.running.length} running` : ''].filter(Boolean);
    const when = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
    // a peer: how it is reached now, when it was paired and last pulled (popover), its address, and Forget
    const peer = me ? '' : `<span class="st tr mono muted" data-testid="transport">${esc(transportLine(c, d))}${info('peer-' + d.id, `${d.direct ? `<b>Paired</b> ${d.paired_at ? esc(when(d.paired_at)) : 'yes'}` : '<b>Not paired</b>: known through the folder only'}<br><b>Last pull</b> ${d.last_pull ? esc(ago(d.last_pull)) : 'never'}${d.address ? `<br><b>Address</b> ${esc(d.address)}` : ''}${d.relayed_by ? `<br><b>Relayed through</b> ${esc(nameOf(c, d.relayed_by))}` : ''}`)}</span>
      <div class="f">${d.direct ? `<input class="input mono" id="addr-${esc(d.id)}" value="${esc(d.address || '')}" placeholder="name:port" maxlength="120" aria-label="Address of ${esc(d.name)}" title="Where to reach ${esc(d.name)} when not on this network: a VPN name, with the port" data-testid="addr-${esc(d.id)}"><button class="btn sm" data-act="peer-addr" data-id="${esc(d.id)}" data-testid="addr-save-${esc(d.id)}">Save</button>` : ''}<button class="btn sm" data-act="peer-forget" data-id="${esc(d.id)}" data-name="${esc(d.name)}" data-testid="forget-${esc(d.id)}">Forget</button></div>`;
    return `<div class="dev" data-testid="computer-${esc(d.id || 'this')}">${me ? '<span class="pill teal me">this one</span>' : ''}<b>${cdot(c, d.id)}${esc(d.name)}</b>
      <span class="st mono ${stale ? 'warnT' : 'acT'}">${esc([me ? { windows: 'Windows', macos: 'macOS', linux: 'Linux' }[st.os] || st.os : d.os, me ? st.version : d.app_version, heard].filter(Boolean).join(' · '))}</span>
      <div class="bar" title="${me ? `${done.toLocaleString()} of ${seq.toLocaleString()} entries published` : `${done.toLocaleString()} of ${seq.toLocaleString()} entries applied here`}"><i class="${pct < 100 && stale ? 'warn' : ''}" style="width:${pct}%"></i></div>
      <span class="st mono muted">${esc(facts.join(' · ') || (seq ? `${seq.toLocaleString()} journal entries` : 'nothing written yet'))}</span>${peer}</div>`;
  };
  el.innerHTML = `${row('This computer', info('devname', 'The name the other computers show for this one, in their lists, on sessions and on the chip at the top.<br><b>Default: the computer name</b> Windows or macOS gives it.'), 'How your other computers see this one.',
      `<input class="input" id="st-devname" value="${esc(c.this.name)}" maxlength="40" aria-label="This computer's name" style="width:220px" data-testid="st-devname"><button class="btn outline" data-act="computers-set" data-key="device_name" data-from="st-devname" data-testid="st-devname-save">Rename</button>`)}
    ${row('How your computers sync', info('syncmode', 'Pick one per computer. Each computer writes its own journal there and reads the others\'; keys and logins never go in it. Switching later keeps everything, because the journal is the same either way.<br><b>Default: off.</b>'), `Pick one per computer. Each computer writes its own journal there and reads the others'; keys and logins never go in it.
      <div class="mcards"><div class="mcard ${on && c.mode === 'folder' ? 'on' : ''}" role="button" tabindex="0" data-act="sync-folder-pick" data-testid="sync-folder" aria-pressed="${on && c.mode === 'folder'}"><i></i><div><b>A shared folder</b><span>OneDrive, Google Drive, an SMB share or a NAS. Nothing to run; the service carries the files.</span>
          <div class="f"><span class="inp mono ${c.folder ? '' : 'muted'}" title="${esc(c.folder)}" data-testid="sync-folder-path">${esc(c.folder || 'No folder chosen yet')}</span><button class="btn sm" data-act="sync-folder-pick" data-testid="sync-folder-pick">${c.folder ? 'Change…' : 'Choose…'}</button>${on && c.mode === 'folder' ? '<button class="btn sm" data-act="sync-off" data-testid="sync-off">Stop syncing</button>' : ''}</div></div></div>
        <div class="mcard ${hubOn ? 'on' : ''}" role="button" tabindex="0" ${hubOn ? '' : 'data-act="hub-join"'} data-testid="sync-hub" aria-pressed="${hubOn}"><i></i><div><b>A hub you host${hubOn ? ` <span class="pill teal" data-testid="hub-state">${hub.ws ? 'live' : 'joined'}</span>` : ''}</b><span>Your VPS with Docker, behind Caddy. Works from any network, no VPN needed, and gives live presence everywhere. Only your computers can read it: everything but their names is sealed with your ring key.</span>
          <div class="f"><span class="inp mono ${hub.url ? '' : 'muted'}" title="${esc(hub.url || '')}" data-testid="hub-url">${esc(hub.url || 'Not joined yet')}</span>${hubOn ? `<button class="btn sm" data-act="hub-invite" data-testid="hub-invite">Invite a computer…</button><button class="btn sm" data-act="hub-leave" data-testid="hub-leave">Stop syncing</button>` : `<button class="btn sm" data-act="hub-join" data-testid="hub-join">Join with a code…</button>`}</div>
          ${hubOn ? `<span class="st mono muted" data-testid="hub-line">joined · ${hubN} ${hubN === 1 ? 'computer' : 'computers'} · ${hub.ws ? 'live' : 'polling'}${hub.ring ? '' : ' · <b class="err">no ring key: enter the recovery key below</b>'}</span>` : ''}</div></div></div>`, '')}
    ${hubOn || hub.ring ? row('Recovery key', info('ring', 'The ring key seals every chunk, presence report and transcript before it goes to the hub, so the hub stores what it cannot read. It is made on the first computer that joins the hub, kept in Credential Manager, carried to paired computers with the pairing, and shown once as the recovery key: 8 groups of 4 letters and digits for your password manager. A computer that is not paired needs it typed in.'), hub.ring ? 'Kept on this computer; paired computers get it with the pairing. Keep the recovery key in your password manager for a computer that is not paired, or a fresh install.' : 'This computer has no ring key, so it cannot read or write the hub: enter the recovery key shown on the computer that first joined, or pair with it.',
      `${hub.ring ? '<button class="btn outline" data-act="ring-show" data-testid="ring-show">Show again</button>' : ''}<button class="btn ${hub.ring ? 'outline' : 'primary'}" data-act="ring-enter" data-testid="ring-enter">Enter recovery key…</button>`) : ''}
    ${row('Carry transcripts', info('transcripts', `Also share this computer's session transcripts, so they can be read and resumed on another computer. A transcript over 20 MB is not carried; the folder holds up to 200 MB of this computer's transcripts, the oldest going first.${ts ? `<br>Carried ${ts.carried} · ${(ts.bytes / 1048576).toFixed(1)} MB${ts.skipped && ts.skipped.length ? ` · ${ts.skipped.length} skipped` : ''}` : ''}<br><b>Default: on.</b>`), 'Share this computer\'s session transcripts, so they can be resumed elsewhere.', `<label class="switch"><input type="checkbox" data-act="computers-set" data-key="sync_transcripts" ${s.sync_transcripts !== false ? 'checked' : ''} data-testid="st-transcripts"> Carry transcripts</label>`)}
    ${row('Presence', info('presence', 'Writes what is running here every minute while the app is open, so the other computers show it in Live routes and Running now.<br><b>Default: on.</b>'), 'Tell the other computers what is running here, every minute while the app is open.', `<label class="switch"><input type="checkbox" data-act="computers-set" data-key="sync_presence" ${s.sync_presence !== false ? 'checked' : ''} data-testid="st-presence"> Share what runs here</label>`)}
    ${row('Direct connection', info('direct', `Finds your other computers on this network (mDNS) or at an address you give (a VPN name), after pairing once with a six-digit code, and talks to them over TLS: what runs there every ten seconds, their history the moment it is written. The folder keeps working underneath; a computer reached neither way is still merged through it.<br><b>Default: off.</b><br>Listens on port <span class="mono" data-testid="listen-port">${c.port || 'chosen at first use'}</span> <button type="button" class="linkb" data-act="listen-port" data-testid="listen-port-change">Change…</button>`), 'On top of the folder or the hub: find the other computers on this network or over the VPN and talk to them directly, for live routes within a second.', `<label class="switch"><input type="checkbox" data-act="computers-set" data-key="direct_connection" ${s.direct_connection ? 'checked' : ''} data-testid="st-direct"> Connect directly</label><button class="btn outline" data-act="pair" data-testid="pair">Pair a computer…</button>`)}
    <div class="devhead"><b>${n} ${n === 1 ? 'computer' : 'computers'}</b><span class="muted sm">${on ? `synced through a ${c.mode === 'hub' ? 'hub' : 'folder'}${c.last_run ? ' · last round ' + ago(c.last_run) : ''}` : 'sync is off: only this computer'}</span>${c.error ? `<span class="err sm" data-testid="sync-error">${esc(c.error)}</span>` : ''}
      <span class="acts">${on ? `<button class="btn sm primary" data-act="computers-now" data-testid="computers-now">${ic.refresh} Sync now</button>` : ''}</span></div>
    <div class="devs" data-testid="computer-cards">${computers(c).map(card).join('')}</div>`;
}
// a switch or the name: saved through computers_set, which answers with the new status
action('computers-set', async (el) => {
  const value = el.type === 'checkbox' ? el.checked : document.getElementById(el.dataset.from).value.trim();
  if (el.type !== 'checkbox' && !value) return;
  if (await mutate(() => call('computers_set', { [el.dataset.key]: value }), 'Saved') === undefined && el.type === 'checkbox') el.checked = !el.checked;
});
action('sync-folder-pick', async () => {
  const p = await pickFolder();
  if (p && await mutate(() => call('computers_set', { sync_mode: 'folder', sync_folder: p }), 'Syncing through ' + p) !== undefined) welcomeBack(p); // phase 4
});
action('sync-off', () => mutate(() => call('computers_set', { sync_mode: 'off' }), 'Sync is off; the folder keeps what was written'));
action('computers-now', async () => {
  const r = await attempt(() => call('computers_now'));
  if (!r) return;
  const n = (r.peers || []).length;
  toast(r.error ? 'Sync round: ' + r.error : `Synced with ${n} other ${n === 1 ? 'computer' : 'computers'}`, !!r.error);
  changed();
});
// phase 2: the port the direct connection listens on, a peer's address, and forgetting a peer
action('listen-port', () => {
  openDialog(`${dlgHead('Listen port')}<div class="df"><label for="lp-v">Port</label><div><input class="input mono" id="lp-v" type="number" min="0" max="65535" value="${(S.cache.computers || {}).port || 0}" data-testid="lp-v"><span class="hint">0 picks a free port once and keeps it. Your other computers find this one by mDNS or by its address with this port.</span></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="lp-ok" data-testid="lp-ok">Save</button></span></div>`);
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
// presence arrives every 10 s while a peer is connected: the cards follow, unless one of them is being edited
document.addEventListener('presence', () => {
  const el = document.getElementById('st-computers');
  if (S.route.view === 'settings' && el && S.state && !document.getElementById('dialog-root').innerHTML && !el.contains(document.activeElement)) drawComputers(S.state.settings, S.state);
});

const kb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
async function drawBackups(s) {
  const [b, ar] = await Promise.all([call('backups').catch(() => null), call('archive_status').catch(() => null)]);
  const el = document.getElementById('st-backups');
  if (!b || !el) return;
  // phase 4: the copy into the sync folder, transcripts in the zip, the archive's age and locations, Restore…
  const folderOn = !!(S.cache.computers && S.cache.computers.mode === 'folder');
  const days = s.archive_days ?? 30, loc = (ar && ar.locations) || [];
  const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });
  el.innerHTML = `${row('Data folder', '', 'Accounts list (unless it lives elsewhere), settings, the usage database, backups and the log.', `<span class="mono muted" style="overflow-wrap:anywhere">${esc(b.data)}</span> <button class="btn outline" data-act="open-folder" data-path="${esc(b.data)}">${ic.folder} Open</button>`)}
    ${row('Automatic backups', info('bk', 'A backup holds the account list, settings, what Harness Manager wrote into each tool, the folders of the skills it manages, and the history: every request, session, limit reading and tag, with the computer each came from, and the archived sessions. API keys and logins stay in Windows Credential Manager and are never in a backup.<br><b>Default: every 24 hours, keeping 10.</b>'), 'Made while the app runs, when the newest is older than this.',
      `<select class="input" data-act="set-now" data-key="backup_hours" style="max-width:170px" aria-label="Backup interval" data-testid="st-bkhours">${[[0, 'Off'], [6, 'Every 6 hours'], [12, 'Every 12 hours'], [24, 'Every 24 hours'], [168, 'Every week']].map(([v, l]) => `<option value="${v}" ${s.backup_hours === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
       <select class="input" data-act="set-now" data-key="backup_keep" style="max-width:130px" aria-label="Backups kept">${[3, 5, 10, 20, 50].map((v) => `<option value="${v}" ${s.backup_keep === v ? 'selected' : ''}>Keep ${v}</option>`).join('')}</select>`)}
    ${row('Also in the sync folder', info('bkfolder', 'Every backup is also copied to <span class="mono">backups\\&lt;this computer&gt;</span> in the sync folder, keeping as many there as here, so a fresh computer finds its history beside the journal.<br><b>Default: on.</b>'), folderOn ? 'A copy of each backup goes beside the journal.' : 'A copy of each backup goes beside the journal, once a sync folder is chosen.', sw('backup_in_sync_folder', s.backup_in_sync_folder !== false, 'Copy backups there', 'st-bk-folder'))}
    ${row('Include transcripts', info('bktr', 'The zip also holds this computer\'s session transcripts, compressed, so a restore brings the conversations back too. Backups get larger.<br><b>Default: off.</b>'), 'Put the session transcripts in each backup.', sw('backup_transcripts', !!s.backup_transcripts, 'Include transcripts', 'st-bk-transcripts'))}
    <div class="bklist"><div class="bkhead"><b>Backups</b> <span class="muted">${b.list.length} · ${kb(b.list.reduce((a, x) => a + x.size, 0))}</span><span class="acts"><button class="btn outline sm" data-act="backup-import" data-testid="backup-import">Import…</button><button class="btn outline sm" data-act="backup-export" data-testid="backup-export">Export…</button><button class="btn sm primary" data-act="backup-now" data-testid="backup-now">${ic.download} Back up now</button></span></div>
      ${b.list.map((x) => `<div class="bkrow" data-testid="backup-${esc(x.label)}"><span class="mono">${esc(stamp(x.at))}</span><span>${esc(x.label)}</span><span class="muted">${kb(x.size)}</span>
        <span class="acts"><button class="iconbtn" data-act="backup-rename" data-name="${esc(x.name)}" data-label="${esc(x.label)}" title="Rename" aria-label="Rename">${ic.edit}</button><button class="iconbtn" data-act="backup-delete" data-name="${esc(x.name)}" title="Delete" aria-label="Delete">${ic.x}</button><button class="btn outline sm" data-act="backup-restore" data-name="${esc(x.name)}" data-testid="restore-${esc(x.label)}">${ic.refresh} Restore</button></span></div>`).join('') || '<p class="empty">No backups yet.</p>'}
      ${(b.in_sync_folder || []).length ? `<div class="bkhead"><b>In the sync folder</b> <span class="muted">${b.in_sync_folder.length} · every computer's</span></div>` + b.in_sync_folder.map((x) => `<div class="bkrow" data-testid="folder-backup-${esc(x.label)}"><span class="mono">${esc(stamp(x.at))}</span><span>${esc(x.label)} <span class="muted">· ${esc(x.device_name)}</span></span><span class="muted">${kb(x.size)}</span>
        <span class="acts"><button class="btn outline sm" data-act="restore-src" data-zip="${esc(x.path)}">${ic.refresh} Restore…</button></span></div>`).join('') : ''}</div>
    ${row('Archive sessions older than', info('arch', 'Transcripts of sessions older than this move from the tools\' folders into the archive, compressed by project and month, once a day while the app runs and with Archive now. Each copy is checked before the original is deleted. The session stays in the list with its numbers and stays readable; it cannot be resumed from the archive. Usage is never touched.<br><b>Default: 30 days.</b> Never keeps every transcript where its tool put it.'), 'Transcripts move from the tools\' folders into the archive, compressed. The session stays in the list and stays readable.',
      `${seg('archive_days', days, [[0, 'Never'], [30, '30 days'], [90, '90 days'], [365, '1 year']], 'st-archive-days')}<input class="input mono" type="number" min="0" max="36500" id="st-archive-custom" value="${days}" style="max-width:80px" aria-label="Days" data-testid="st-archive-custom"><button class="btn outline sm" data-act="archive-days-custom" data-testid="st-archive-custom-save">days</button>`)}
    ${row('Archive location', info('archloc', 'Where the copies go: the sync folder (<span class="mono">archive\\&lt;this computer&gt;</span>, so every computer and a fresh install can read them), a folder of your choice, or both.<br><b>Default: the sync folder.</b> With neither, nothing is archived.'), 'In the sync folder, a folder of your choice, or both.',
      `<label class="switch"><input type="checkbox" data-act="set-now" data-key="archive_in_sync_folder" ${s.archive_in_sync_folder !== false ? 'checked' : ''} data-testid="st-archive-sync"> The sync folder${folderOn ? '' : ' (none chosen yet)'}</label>
       <span class="inp mono ${s.archive_dir ? '' : 'muted'}" title="${esc(s.archive_dir)}" data-testid="st-archive-dir">${esc(s.archive_dir || 'No folder chosen')}</span><button class="btn outline sm" data-act="archive-dir-pick" data-testid="st-archive-dir-pick">${s.archive_dir ? 'Change…' : 'Choose…'}</button>${s.archive_dir ? '<button class="btn outline sm" data-act="archive-dir-clear" data-testid="st-archive-dir-clear">None</button>' : ''}`)}
    ${row('Archive now', '', ar ? `${ar.archived} archived · ${kb(ar.bytes)}${loc.map((l) => ` · ${l.kind === 'sync_folder' ? 'sync folder' : 'folder'}: ${l.sessions}${l.on ? '' : ' (off)'}`).join('')}${ar.last && ar.last.at ? ` · last run ${esc(when(ar.last.at))}` : ''}` : 'Nothing archived yet.', `<button class="btn outline" data-act="archive-now" data-testid="archive-now">Archive now</button>`)}
    ${row('Restore', info('restore', 'Merges the history in a backup (this computer\'s, or one in the sync folder) or in the sync folder itself into this computer: requests by their id, sessions, limit history, tags, other computers and archived transcripts. Nothing is counted twice; restoring again adds nothing. With a record of this computer\'s name in it, you can continue as that computer. Keys and logins are never in a backup.'), 'Merge a backup or the sync folder\'s history into this computer. Nothing is counted twice.', `<button class="btn outline" data-act="restore-pick" data-testid="restore-open">Restore…</button>`)}`;
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
  openDialog(`${dlgHead('Rename backup')}<div class="df"><label for="bk-l">Label</label><div><input class="input" id="bk-l" value="${esc(el.dataset.label)}" maxlength="60"></div></div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close">Cancel</button><button class="btn primary" id="bk-ok">Rename</button></span></div>`);
  document.getElementById('bk-ok').onclick = async () => { if (await attempt(() => call('backup_rename', { name: el.dataset.name, label: document.getElementById('bk-l').value }))) { closeDialog(); changed(); } };
});
action('backup-restore', async (el) => {
  if (!(await confirmDialog('Restore this backup?', 'The account list, settings and what Harness Manager wrote into each tool go back to this backup, and skill folders missing from the store come back. The state now is kept as a backup first. API keys and logins are not touched.', 'Restore', false))) return;
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
  if (!(await confirmDialog('Restore from this file?', `Everything in <span class="mono">${esc(from)}</span> is put back, as for a backup made here. The state now is kept as a backup first.`, 'Restore', false))) return;
  const r = await attempt(() => call('backup_import', { path: from }));
  if (r) restored(r);
});
action('log-open', () => attempt(() => call('log_open')));
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
  el.disabled = true;
  el.textContent = 'Updating…';
  // on success the app restarts, so only a failure comes back, or "nothing newer" when the offer was out of date
  const v = await attempt(() => call('self_update_install'));
  if (!v) { el.disabled = false; el.textContent = 'Update and restart'; return; }
  if (!v.version) { setSelfUpdate(v); toast(`Harness Manager ${v.current} is the newest version`); render(); }
});
action('set-now', async (el) => {
  // a switch's state, a segment's value, or a dropdown's choice (numbers as numbers)
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
  mutate(() => call('settings_set', { key: el.dataset.key, value }), 'Saved');
});
action('import-ps', async () => { const r = await mutate(() => call('import_ps', {})); if (r) { toast(`Imported ${r.accounts.length} accounts and ${r.keys.length} keys${r.skipped.length ? '; skipped ' + r.skipped.length : ''}`); } });
action('detect', async () => { const r = await mutate(() => call('detect', {})); if (r) toast(r.main_profiles.length ? `Added ${r.main_profiles.join(', ')}` : 'No new tools found'); });
action('relink', async () => { const r = await mutate(() => call('sync', { relink: true })); if (r) toast(r.relinked.length ? `Linked: ${r.relinked.join('; ')}` : 'Nothing to link; accounts are in step'); });

// ---------- CLI reference ----------
const CLI = [
  ['Accounts', [['list', 'Accounts with plan, limits, tokens and open consoles'], ['show', 'One account in detail'], ['add', 'Add an account (subscription login by default)'],
    ['login', 'Log an account in'], ['remove', 'Remove an account; --delete-home also deletes its folder'], ['key', 'API keys, kept in Windows Credential Manager']]],
  ['Run', [['run', 'Run the tool as an account in this console'], ['start', 'Open a new terminal running the account'], ['resume', 'Reopen a session in its own account'], ['open', 'Open the desktop app']]],
  ['Usage and limits', [['usage', 'Token usage per account and day'], ['overview', 'Cost at list prices, requests, tokens, cache hits'], ['requests', 'Every model reply, newest first'], ['quota', 'Limits of one account; --live asks Anthropic now']]],
  ['Sessions and projects', [['sessions', 'Sessions of every tool, grouped by project, date, account, tool or tag; --open, --project, --tag'], ['session', 'Print one session as a transcript'], ['projects', 'List projects with tokens and cost, rename or merge them'], ['tag', 'Tag a session or a project'], ['tags', 'Every tag with its counts']]],
  ['Shared setup', [['sync', 'Bring every account in step'], ['mcp', 'MCP servers shared across tools'], ['skills', 'Skills shared across tools'], ['prompts', 'Prompt files per tool']]],
  ['Routing and setup', [['provider', 'Router providers: endpoints and keys'], ['route', 'Failover or aggregate routes'], ['router', 'Start, stop or check the local router'], ['settings', 'Read or change settings'],
    ['init', 'Find installed tools and add their main profiles'], ['import', 'Import from the old claude-accounts.ps1 launcher']]],
  ['Computers', [['computers status', 'This computer, the shared folder and the others; --json'], ['computers now', 'Publish what is new here and merge what the others published'], ['computers set', 'The name, the folder, what is shared, --direct on|off, --port'],
    ['hub join <url> --code <c> | --admin <t>', 'Join the hub you host: with a code from a computer already on it, or the admin token for the first'], ['hub invite', 'A one-time six-digit code for another computer'], ['hub status', 'Joined, computers, the last push and pull; --json'], ['hub leave', 'Leave the hub; what was merged stays'],
    ['ring show | enter [key]', 'The recovery key: print it for your password manager, or enter it on a computer that is not paired'],
    ['computers pair', 'Show a six-digit code and wait for the other computer; --host for one on another network'], ['computers join <code>', 'Pair with the computer showing the code; --host name:port when not on this network'],
    ['computers peers', 'The paired computers, how each is reached and when it was last pulled'], ['computers forget <id or name>', 'Stop connecting to a computer; what it sent stays'], ['computers serve --seconds <n>', 'Run the listener without the app, for tests and headless use']]],
];
view('cli', async (sheet) => {
  sheet.className = 'sheet';
  const ex = [['hrm list', 'accounts, plans, limits, tokens, open consoles'], ['hrm start work-max --cwd D:\\Projects\\my-app', 'a new terminal, in that folder'],
    ['hrm quota work-max --live', 'limits straight from Anthropic'], ['hrm sessions --group project --json', "every tool's sessions, for scripts"]];
  const w = Math.max(...ex.map((e) => e[0].length)) + 2;
  sheet.innerHTML = `<div class="crumb">CLI reference</div><h1><span class="mono">hrm</span> does everything this window does.</h1><p class="find">Add <span class="mono">--json</span> to any command for scripts.</p>
    <pre class="panel cliex mono">${ex.map(([c, d]) => `${esc(c.padEnd(w))}<span class="muted"># ${esc(d)}</span>`).join('\n')}</pre>
    <div class="cligrid">${CLI.map(([g, rows]) => `<section class="panel"><h2>${esc(g)}</h2>${rows.map(([c, d]) => `<div class="clir"><span class="mono acT">${esc(c)}</span><span>${esc(d)}</span></div>`).join('')}</section>`).join('')}</div>`;
});

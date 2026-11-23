// Sessions (the list) and Session (the reading view), fed by the index: `sessions`, `projects`, `tag_list`,
// `computers_status`, `transcript`, `session_stats`, `file_stamp`. The list is the 1.10 drawing's (Option A, with the
// middle zone of docs/project-review-2026-11/mockups/b-sessions.html): one filter line, Running now, then the groups
// in one card with 44 px rows. A click on a row selects the session and shows it in the pane at the right (pane.js;
// the drawing's zone 3); its page stays one step away. The tags shared with Projects and Accounts live here too.
// Every redraw goes through patch(), so a live arrival keeps the reader's scroll, focus and open folds.

import { S, call, view, go, toast, attempt, mutate, changed, acctName, acctEmail, openDialog, closeDialog, dlgHead, openMenu, action, render, saveFile, patch, drawPins, loadState, shown, needsMark, needsGroupMark, runOf, clearNotice } from './core.js';
import { esc, fmt, hm, when, dur, toolIcon, ic, md } from './lib.js';
import { computersStatus, syncOn, nameOf, cdot } from './computers.js';
import { resumeData, reasonShort, canResume } from './resume.js';
import { nameEl, pinBtn, area, refreshProjects, tellApart } from './projects.js';
import './restore.js';
import { pageHead, detailHead, seg, plural } from './parts.js';
import { paneOf, selected, paneOpen, rowAttrs, paneHtml, patchPane, drawWithPane } from './pane.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };

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
const clean = (p) => Object.fromEntries(Object.entries(p).filter(([, v]) => v !== '' && v != null));
const sq = (patchParams) => go('sessions', clean({ ...S.route.params, ...patchParams }));
export const href = (view, p) => '#' + view + (Object.keys(clean(p)).length ? '?' + new URLSearchParams(clean(p)) : '');
export const money = (c, d = 2) => (c == null ? '—' : '$' + c.toLocaleString(undefined, { minimumFractionDigits: d, maximumFractionDigits: d }));
const sMore = new Set(), sFold = new Set();
// what the Project and Tag pickers list: the data of the last draw, and whether every project was asked for
let pick = null, pickMore = false;
// the page reads the index in pages of 50 rows; the search, the computer and every filter are the backend's
const PER = 50;
/** A segmented choice that sets one parameter of the hash. */
const sseg = (k, opts, cur, label, tid) => seg(label, opts, cur, (v) => `data-act="s-set" data-k="${k}" data-v="${esc(v)}" data-testid="${esc(tid(v))}"`);

/** A session from another computer: its account, its login and its log file are that computer's. */
const isRemote = (s, pcs) => !!(pcs && pcs.this && s.device && s.device !== pcs.this.id);
/** The address of a session's full page. */
const sessionHash = (s, pcs) => href('session', { profile: s.profile, id: s.id, device: isRemote(s, pcs) && !s.archived ? s.device : '' });
/** One row, 44 px: status dot, title (a failed one says so in words) with its tags, account (its tool's icon first),
 * computer (sync on), tokens, when, one action. `running`: a row of Running now, which says when it started.
 * A click selects the row (pane.js); `sel` is the selected session's key. `paned`: the pane is open beside the list,
 * so the row is the drawn columns only and its action is the pane's. */
function sessionRow(s, pcs, running, sel = '', paned = false) {
  const sync = syncOn(pcs), remote = isRemote(s, pcs), key = s.profile + '/' + s.id;
  const device = s.device_name || nameOf(pcs, s.device), far = s.open_elsewhere ? device : '';
  // the login is this computer's: on another computer the same account name is often another login
  const mail = remote ? '' : acctEmail(s.profile);
  const path = s.cwd ? s.cwd.split(/[\\/]/).slice(-2).join('\\') : '';
  // where it ran (its messages and the last two folders): shown when the title leaves it room, whole on hover
  // archived: read only; open: follow; from another computer: resume here when its repository and account are here, else why not
  const act = s.archived ? '' : s.open ? `<button class="btn sm" data-act="open-session" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" ${remote ? `data-device="${esc(s.device)}"` : ''}>Follow</button>`
    : remote ? (canResume(s.resumable) ? `<button class="btn sm" data-act="resume-here" data-s="${resumeData(s)}" data-testid="resume-here-${esc(s.id)}">Resume here</button>` : `<span class="a-reason" title="${esc(s.resumable || '')}" data-testid="reason-${esc(s.id)}">${esc(reasonShort(s.resumable) || 'on ' + device)}</span>`)
    : `<button class="btn sm" data-act="resume" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-cwd="${esc(s.cwd)}">Resume</button>`;
  const at = running ? when(s.started_at) : far ? 'running on ' + far : s.open ? 'live' : when(s.last_active_at);
  return `<div class="a-srow" ${rowAttrs(key, sessionHash(s, pcs), key === sel)} data-key="row:${esc(s.profile)}/${esc(s.id)}" data-testid="session-${esc(s.id)}"><i class="${s.open ? 'live-pulse-dot' : 'a-dot' + (s.failed ? ' bad' : '')}" aria-hidden="true"></i>
    <div class="a-tcell"><button class="a-title" data-sel-title ${key === sel ? 'aria-current="true"' : ''} data-testid="read-${esc(s.id)}" title="${esc(s.cwd || 'No folder')}"><b>${esc(s.title || 'Untitled session')}</b></button>${remote ? '' : needsMark(s.profile, s.id)}${s.failed ? `<span class="a-fail" title="${plural(s.failed, 'failed command')}">${s.failed} failed</span>` : ''}${s.archived ? `<span class="a-pill arch" data-testid="archived-${esc(s.id)}">archived</span>` : ''}${s.continued_from ? `<span class="a-pill arch" data-testid="continued-${esc(s.id)}">continued from ${esc(s.continued_from)}</span>` : ''}${tagsHtml(s.tags, 'session', s.profile + '/' + s.id, s.title || 'this session')}${paned ? '' : `<span class="a-where" title="${esc(plural(s.messages, 'message') + (s.cwd ? ' · ' + s.cwd : ''))}"><span class="a-wtx">${plural(s.messages, 'msg')}${path ? ` · <span class="mono">${esc(path)}</span>` : ''}</span></span>`}</div>
    <span class="a-acct" title="${esc((TOOL[s.harness] || s.harness) + ' · ' + s.profile + (mail ? ' · logged in as ' + mail : ''))}" data-testid="acct-${esc(s.id)}">${toolIcon(s.harness, 16)}<span class="a-acctwho"><span>${esc(acctName(s.profile))}</span>${mail && !paned ? `<small>${esc(mail)}</small>` : ''}</span></span>${sync ? `<span class="a-pc" title="Ran on ${esc(device)}">${cdot(pcs, s.device, device, 'on')}<span>${esc(device)}</span></span>` : ''}
    <span class="num">${fmt(s.tokens)}</span><span class="a-when ${!running && (s.open || far) ? 'live' : ''}" title="${esc(at)}">${esc(at)}</span>${paned ? '' : `<span class="a-act">${act}</span>`}</div>`;
}

// what the list last read: a choice that only changes what is shown (a group folded) draws from it again, without
// asking the backend
let last = null;
/** The reads of the list, as one text: two addresses that differ only in what is shown share it. */
const listKey = (params) => JSON.stringify(Object.entries(params).filter(([k]) => k !== 'sel').sort());
/** The filters of the address, as the page reads them. pc: every computer (''), this one ('here'), or one other
 * computer (its id); all answered by the index. */
const filtersOf = (params) => ({ group: params.group || 'project', days: params.days || '7', live: params.live === '1', project: params.project || '', q: params.q || '', tag: params.tag || '', pc: params.pc || '', page: +params.page || 0 });
view('sessions', async (sheet, params) => {
  const { group, days, live, project, q, tag, pc, page } = filtersOf(params);
  // the same list as the one last read: drawn from that at once. When the list is on screen and only the selection
  // moved, that is all; when the reader comes back to it (from a session's page), the fresh read follows
  if (last && last.key === listKey(params)) {
    const only = !!sheet.querySelector('[data-tour=sessions]') && (last.params.sel || '') !== (params.sel || '');
    last.params = params;
    sheet.className = 'sheet';
    draw(sheet);
    if (only) return;
  }
  if (!S.state) await loadState();
  const scope = pc === 'here' ? 'here' : 'all';
  const filter = { harness: params.harness || null, profile: params.profile || null, days: days === 'all' || live ? null : +days, query: q || null, open: live || null, project: project || null, tag: tag || null, device: pc && pc !== 'here' ? pc : null, per: PER, page };
  sheet.className = 'sheet';
  // the list already on screen stays while the fresh one is read; from another page, a line says what is coming
  if (!sheet.querySelector('[data-tour=sessions]')) sheet.innerHTML = `<div class="page-body">${pageHead('sessions', 'Reading sessions…')}</div>`;
  let [ans, plist, tlist, pcs] = await Promise.all([call('sessions', { filter, group, scope }), call('projects', { days: days === 'all' ? null : +days }), call('tag_list').catch(() => []), computersStatus().catch(() => null)]);
  S.cache.tags = tlist;
  drawPins(plist);
  const groups = ans.groups;
  // a group opened in full fetches its own rows: the page carries at most 50 rows across every group
  const gfilter = (k) => ({ ...filter, per: 1000, page: 0, ...{ project: { project: k }, account: { profile: k }, tool: { harness: k }, tag: { tag: k }, date: { day: k } }[group] });
  await Promise.all(groups.filter((g) => sMore.has(group + ':' + g.key) && g.sessions.length < g.total).map(async (g) => {
    const r = await call('sessions', { filter: gfilter(g.key), group, scope }).catch(() => null);
    const full = r && r.groups.find((x) => x.key === g.key);
    if (full) g.sessions = full.sessions;
  }));
  last = { key: listKey(params), params, ans, plist, tlist, pcs };
  draw(sheet);
});
/** Draw the list again from what was last read: after a choice that only changes what is shown. */
function show() {
  if (last && S.route.view === 'sessions' && S.drawn === location.hash) draw(document.getElementById('sheet')); else render();
}
function draw(sheet) {
  const { params, ans, plist, tlist, pcs } = last;
  const { group, days, live, project, q, tag, pc, page } = filtersOf(params);
  let groups = ans.groups;
  const total = ans.total, sync = syncOn(pcs);
  const all = groups.flatMap((g) => g.sessions);
  // the selected session, and whether the pane shows it: a session the list does not hold (a link, another page of
  // the list) is shown from its transcript once that is read
  const sel = params.sel || '', paned = !!sel && paneOpen('sessions');
  const picked = sel ? all.find((s) => s.profile + '/' + s.id === sel) || (pn.key === sel && pn.t ? pn.t.meta : null) : null;
  last.picked = picked;
  // one primary on the screen: with the pane open it is the pane's, and the head's New session is a secondary
  const pri = paned ? '' : 'primary';
  // a running session sits at the top, whatever the grouping, never inside a fold
  const isLive = (s) => s.open || s.open_elsewhere;
  const running = all.filter(isLive);
  if (!live) groups = groups.map((g) => ({ ...g, sessions: g.sessions.filter((s) => !isLive(s)), total: g.total - g.sessions.filter(isLive).length })).filter((g) => g.sessions.length || g.total);
  const filtered = !!(q || tag || live || pc || params.profile || params.harness);
  const cur = project ? plist.find((p) => p.key === project) || { key: project, name: (all[0] && all[0].project_name) || project, sessions: total, tokens: all.reduce((a, s) => a + s.tokens, 0), accounts: [...new Set(all.map((s) => s.profile))], live: running.length } : null;
  if (!total && !filtered && !cur && days === 'all') {
    drawWithPane(sheet, `<div class="page-body" data-tour="sessions">${pageHead('sessions', 'No session yet.', '<button class="btn primary" data-act="session-new" data-testid="session-new">New session</button>')}<section class="panel-card empty-state"><span class="muted">Sessions appear here as your tools write their logs.</span></section></div>`, 'sessions:' + last.key);
    return;
  }
  const keep = clean({ group: params.group, days: params.days });
  pick = { plist, tlist, keep, live, tag, liveN: running.length };
  // a project is named with what tells it from another of the same name: its remote, or the last folders of its path
  const byProject = group === 'project', apart = byProject ? tellApart([...new Map([...plist, ...groups].map((p) => [p.key, p])).values()]) : null;
  // the head of a card of rows: its name and a note, then the names of the columns, on the rows' own grid
  const listHead = (title, note, whenLabel) => `<div class="a-lh"><div><h2 class="section">${title}</h2><span class="small muted trunc">${note}</span></div><span class="label">Account</span>${sync ? '<span class="label">Computer</span>' : ''}<span class="label num">Tokens</span><span class="label num">${whenLabel}</span>${paned ? '' : '<span></span>'}</div>`;
  // one group: a 36 px band (fold, name, what tells it apart, its numbers, its actions), then its rows
  const grp = (g) => {
    const k = group + ':' + g.key, open = !sFold.has(k), n = g.total || g.sessions.length;
    const p = byProject && g.key ? plist.find((x) => x.key === g.key) : null, hint = byProject ? apart.get(g.key).hint : '';
    return `<div class="a-sec ${open ? '' : 'shut'}" data-key="grp:${esc(k)}" data-testid="group"><div class="a-gh"><button class="a-fold" data-act="s-fold" data-k="${esc(k)}" aria-expanded="${open}" aria-label="${open ? 'Fold' : 'Open'} ${esc(g.name)}">${open ? '▾' : '▸'}</button>
      <h2 class="a-gname">${group === 'tag' && g.key ? `<span class="a-tag ${tagColor(g.key)}">${esc(g.name)}</span>` : esc(g.name)}</h2>${hint ? `<span class="a-path mono" title="${esc(g.root || g.remote)}" data-testid="group-hint">${esc(hint)}</span>` : ''}${open ? '' : needsGroupMark(g.sessions.map((x) => x.profile + '/' + x.id))}
      <span class="a-tot">${plural(n, 'session')} · ${fmt(g.tokens)} tokens${p && p.cost != null ? ' · ' + money(p.cost) + ' ≈' : ''}</span><span class="grow"></span>
      ${byProject && g.root ? `<button class="btn sm quiet" data-act="project-new" data-dir="${esc(g.root)}" data-name="${esc(g.name)}" data-profile="${esc(g.sessions[0] ? g.sessions[0].profile : '')}" data-testid="project-new">New session</button>` : ''}${byProject && g.key ? `<button class="a-ib" data-act="project-menu" data-key="${esc(g.key)}" data-name="${esc(g.name)}" data-pinned="${p && p.pinned ? 1 : ''}" aria-label="Open, pin, rename or merge ${esc(g.name)}" aria-haspopup="menu">${ic.dots}</button>` : ''}</div>
      ${open ? g.sessions.map((s) => sessionRow(s, pcs, false, sel, paned)).join('') + (g.sessions.length < n ? `<div class="a-more-row"><button class="btn sm quiet" data-act="s-more" data-k="${esc(k)}" data-testid="s-more-${esc(g.key)}">Show all ${plural(n, 'session')}</button></div>` : '') : ''}</div>`;
  };
  const head = cur
    // a project: the back line, its own name (renamed in place) with its pin and tags, where it lives, and its numbers under the head
    ? `${detailHead({ back: ['#projects', 'Projects'], titleAttrs: 'data-testid="project-title"',
      title: `${nameEl(cur)}<button class="a-ib" data-act="project-rename" data-key="${esc(cur.key)}" title="Rename (or double-click the name)" aria-label="Rename ${esc(cur.name)}">${ic.edit}</button>${pinBtn(cur)}${tagsHtml(cur.tags, 'project', cur.key, cur.name)}`,
      status: `<span class="mono a-path">${esc(cur.remote || cur.root || '')}</span>`,
      actions: `<button class="btn icon" data-act="project-menu" data-key="${esc(cur.key)}" data-name="${esc(cur.name)}" data-pinned="${cur.pinned ? 1 : ''}" aria-label="Pin, rename or merge" aria-haspopup="menu">⋯</button>${cur.root ? `<button class="btn ${pri}" data-act="project-new" data-dir="${esc(cur.root)}" data-name="${esc(cur.name)}" data-profile="${esc(all[0] ? all[0].profile : '')}" data-testid="project-new">New session</button>` : ''}` })}
        <div class="a-pstats" data-testid="project-stats"><span><b>${money(cur.cost)}</b> ≈ list prices</span><span><b>${fmt(cur.tokens)}</b> tokens</span><span><b>${cur.sessions}</b> ${cur.sessions === 1 ? 'session' : 'sessions'}</span><span><b>${(cur.accounts || []).length}</b> ${(cur.accounts || []).length === 1 ? 'account' : 'accounts'}</span>${cur.live ? `<span class="a-live"><b>${cur.live}</b> running</span>` : ''}</div>`
    // the list: what runs now, then how many sessions the range (or the search) holds
    : pageHead('sessions', `${running.length ? `${running.length} running now` : 'No session is running'}; <span data-testid="sessions-count">${total.toLocaleString()} ${filtered ? (total === 1 ? 'matches ' : 'match ') : ''}${days === 'all' || live ? 'in all' : 'in the last ' + esc(days) + ' days'}</span>.`,
      `<button class="btn ${pri}" data-act="session-new" data-testid="session-new">New session</button>`);
  // a project's view lists no other project: its own accounts and tags are choices in the line, and a panel about the
  // project appears only when it spans several folders or accounts
  const folders = cur ? [...new Set(all.map((s) => s.cwd).filter(Boolean))] : [];
  const accts = cur ? (cur.accounts && cur.accounts.length ? cur.accounts : [...new Set(all.map((s) => s.profile))]) : [];
  const ptags = cur ? [...new Set([...(cur.tags || []), ...all.flatMap((s) => s.tags || [])])] : [];
  const about = cur && (folders.length > 1 || accts.length > 1) ? `<aside class="a-about" aria-label="This project" data-testid="project-about">
      <div class="a-rail-sec">This project</div><div class="a-kv"><b>${money(cur.cost)}</b><span>≈ list prices</span><b>${fmt(cur.tokens)}</b><span>tokens</span><b>${cur.sessions}</b><span>${cur.sessions === 1 ? 'session' : 'sessions'}</span></div>
      ${cur.series && cur.series.some((v) => v) ? `<div class="a-rail-sec">${days === 'all' ? 'Activity' : esc(days) + ' days'}</div>${area(cur.series)}` : ''}
      ${running.length ? `<div class="a-rail-sec">Running</div>${running.map((s) => `<a class="a-rail-item" href="#session?profile=${encodeURIComponent(s.profile)}&id=${encodeURIComponent(s.id)}"><i class="a-dot live"></i><span>${esc(s.title || 'Untitled session')}</span><span></span></a>`).join('')}` : ''}
      ${accts.length > 1 ? `<div class="a-rail-sec">Accounts</div>${accts.map((a) => { const n = all.filter((s) => s.profile === a).length; return `<div class="a-arow"><span>${esc(acctName(a))}</span><span>${n}${all.length ? ' · ' + Math.round((n / all.length) * 100) + '%' : ''}</span></div>`; }).join('')}` : ''}
      ${folders.length > 1 ? `<div class="a-rail-sec">Folders</div>${folders.slice(0, 6).map((f) => `<div class="a-afold mono" title="${esc(f)}">${esc(f)}</div>`).join('')}${folders.length > 6 ? `<div class="a-afold">${folders.length - 6} more</div>` : ''}` : ''}</aside>` : '';
  // the one filter line: the grouping, the range, the Project and Tag pickers (in a project: its accounts and tags),
  // the filters in force as chips with ✕, and at the right the computer when sync is on
  const pcBtn = (v, label, title, dot = '') => `<button type="button" class="${pc === v ? 'on' : ''}" aria-pressed="${pc === v}" data-act="s-set" data-k="pc" data-v="${esc(v)}" title="${esc(title)}" data-testid="pc-${esc(v || 'all')}">${dot}${esc(label)}</button>`;
  const fbar = `<div class="a-fbar" data-tour="filters" data-testid="filters"><span class="label">Group by</span>${sseg('group', [['project', 'Project'], ['date', 'Date'], ['account', 'Account'], ['tool', 'Tool'], ['tag', 'Tag']].filter(([k]) => !(cur && k === 'project')), group, 'Group by', (v) => 'group-' + v)}${sseg('days', [['7', '7 d'], ['30', '30 d'], ['all', 'All']], days, 'Range', (v) => 'days-' + v)}
      ${cur ? `${accts.length > 1 ? `<span class="label">Accounts</span>${sseg('profile', [['', 'All'], ...accts.map((a) => [a, acctName(a)])], params.profile || '', 'Accounts', (v) => 'pacct-' + (v || 'all'))}` : ''}${ptags.length ? `<span class="label">Tags</span>${sseg('tag', [['', 'All'], ...ptags.map((t) => [t, t])], tag, 'Tags', (v) => 'ptag-' + (v || 'all'))}` : ''}`
    : `<button class="btn sm" data-act="s-pick-project" aria-haspopup="menu" data-testid="pick-project">Project ▾</button>${tlist.length ? '<button class="btn sm" data-act="s-pick-tag" aria-haspopup="menu" data-tour="tags" data-testid="pick-tag">Tag ▾</button>' : ''}`}
      ${live ? `<button class="a-chipx" data-act="s-set" data-k="live" data-v="" data-testid="chip-live">Running now ✕</button>` : ''}${tag && !cur ? `<button class="a-chipx" data-act="s-set" data-k="tag" data-v="" data-testid="chip-tag"><span class="a-tag ${tagColor(tag)}">${esc(tag)}</span> ✕</button>` : ''}${q ? `<button class="a-chipx" data-act="s-set" data-k="q" data-v="" title="The search above searches every session" data-testid="chip-q">“${esc(q)}” ✕</button>` : ''}
      ${sync ? `<span class="grow"></span><div class="seg" role="group" aria-label="Computer" data-testid="pc-seg">${pcBtn('', 'All', 'All computers')}${pcBtn('here', 'This', 'This computer: ' + pcs.this.name, cdot(pcs, pcs.this.id, pcs.this.name, 'on'))}${pcs.peers.length <= 3 ? pcs.peers.map((x) => pcBtn(x.id, x.name, x.name, cdot(pcs, x.id, x.name, 'on'))).join('') : ''}</div>${pcs.peers.length > 3 ? `<button class="btn sm" data-act="s-pc-menu" aria-haspopup="menu" data-testid="pc-other">${esc(pc && pc !== 'here' ? nameOf(pcs, pc) : 'Elsewhere')} ▾</button>` : ''}` : ''}</div>`;
  const here = running.filter((s) => !s.open_elsewhere).length;
  const body = `${running.length && !live ? `<section class="a-grp livegrp" data-key="grp:live" data-testid="group">${listHead('Running now', `${running.length}${sync ? ` · ${here} here, ${running.length - here} elsewhere` : ''}`, 'Started')}${running.map((s) => sessionRow(s, pcs, true, sel, paned)).join('')}</section>` : ''}
    ${groups.length ? `<section class="a-grp" data-key="grp:list">${listHead('By ' + esc(group), plural(groups.length, { project: 'project', date: 'day', account: 'account', tool: 'tool', tag: 'tag' }[group] || 'group'), 'Last active')}${groups.map(grp).join('')}</section>`
    : running.length && !live ? '' : `<section class="a-grp"><div class="a-empty">No sessions match.<button class="btn sm" data-act="s-clear">Clear filters</button></div></section>`}
    ${total > PER ? `<div class="a-pager" data-testid="s-pager"><button class="btn sm icon" data-act="s-set" data-k="page" data-v="${page - 1}" ${page === 0 ? 'disabled' : ''} aria-label="Previous page" data-testid="page-prev">‹</button><span>${page * PER + 1}–${Math.min(total, (page + 1) * PER)} of ${total}</span><button class="btn sm icon" data-act="s-set" data-k="page" data-v="${page + 1}" ${(page + 1) * PER >= total ? 'disabled' : ''} aria-label="Next page" data-testid="page-next">›</button></div>` : ''}`;
  drawWithPane(sheet, `<div class="page-body" data-tour="sessions">${head}${fbar}<div class="a-wrap ${about ? '' : 'one'}">${about}<div class="a-smain ${sync ? '' : 'nopc'} ${paned ? 'paned' : ''}" data-tour="groups">${body}</div></div></div>${
    paned ? paneHtml(...(picked ? paneParts(picked, pcs) : ['Session', '<div class="grow name">Session</div>', `<div class="pane-sec"><p class="pane-note" data-testid="pane-rest">${pn.key === sel && pn.gone ? 'This session is not in the list, and its transcript could not be read.' : 'Reading the session…'}</p></div>`])) : ''}`, 'sessions:' + last.key);
  if (paned) followPane(sel, picked, pcs);
  // the top bar's search is the index's: a pause in typing asks for the matching sessions
  const find = document.getElementById('find');
  if (q && !find.value.trim()) find.value = q; // searched from another page
  let timer;
  S.pageFind = (v) => { clearTimeout(timer); timer = setTimeout(() => { if ((v || '').trim() !== q) sq({ q: (v || '').trim(), page: '' }); }, 250); };
  S.pageFind(find.value); // typed while the page was being drawn (a redraw clears the page's search until it is set again)
}
action('s-set', (el) => {
  if (el.dataset.k === 'q') document.getElementById('find').value = el.dataset.v;
  sq({ [el.dataset.k]: el.dataset.v, ...(el.dataset.k === 'page' ? {} : { page: '' }) });
});
// from four other computers on, they are one button that opens a menu with every one of them (the same pc parameter)
action('s-pc-menu', (el) => { const pcs = S.cache.computers, pc = S.route.params.pc || ''; openMenu(el, pcs.peers.map((x) => ({ html: cdot(pcs, x.id, x.name, 'on') + esc(x.name), on: x.id === pc, testid: 'pc-' + x.id, run: () => sq({ pc: x.id, page: '' }) }))); });
// The Project and Tag pickers: what the page's own project rail listed, as two menus. An item is a dot, a name and a
// count; `sep` draws a line above it, where a new part of the menu starts.
const pk = (dot, name, count, cls = '') => `<span class="a-pk ${cls}">${dot || '<i></i>'}<span class="a-pkn">${name}</span>${count}</span>`;
const cnt = (v, live = false) => `<span class="a-cnt ${live ? 'live' : ''}">${v}</span>`;
action('s-pick-project', (el) => {
  const { plist, keep, live, liveN } = pick, apart = tellApart(plist);
  const pinned = plist.filter((p) => p.pinned), rest = plist.filter((p) => !p.pinned), some = pickMore ? rest : rest.slice(0, 8);
  // a pinned project carries the pin; two projects of one name carry what tells them apart
  const item = (p, i) => ({ html: pk(`<i class="a-dot ${p.live ? 'live' : ''}"></i>`, `${p.pinned ? ic.pin : ''}<span class="trunc">${esc(p.name)}</span>${apart.get(p.key).dup ? `<small class="trunc">${esc(apart.get(p.key).hint)}</small>` : ''}`, cnt(p.live ? p.live + ' live' : p.sessions, p.live), i ? '' : 'sep'),
    testid: 'pick-' + p.key, run: () => go('sessions', { ...keep, project: p.key }) });
  openMenu(el, [
    { html: pk('<i class="a-dot live"></i>', 'Live now', cnt(liveN || '', true)), on: live, testid: 'pick-live', run: () => go('sessions', { ...keep, live: '1' }) },
    { html: pk('<i class="a-dot"></i>', 'All projects', cnt(plist.reduce((a, p) => a + p.sessions, 0))), on: !live, testid: 'pick-all', run: () => go('sessions', keep) },
    ...pinned.map(item), ...some.map(item),
    ...(some.length < rest.length ? [{ html: pk('', `<span class="muted">${rest.length - some.length} more…</span>`, ''), testid: 'pick-more', run: () => { pickMore = true; el.click(); } }] : []),
    { html: pk('', 'Every project, with costs →', '', 'sep'), testid: 'pick-projects', run: () => go('projects') },
  ]);
});
action('s-pick-tag', (el) => openMenu(el, pick.tlist.map((t) => ({ html: pk('', `<span class="a-tag ${tagColor(t.tag)}">${esc(t.tag)}</span>`, cnt(t.sessions + t.projects)), on: pick.tag === t.tag, testid: 'pick-tag-' + t.tag, run: () => go('sessions', { ...pick.keep, tag: t.tag }) }))));
action('s-clear', () => { document.getElementById('find').value = ''; sq({ live: '', tag: '', pc: '', profile: '', harness: '', q: '', page: '' }); });
action('s-more', (el) => { sMore.add(el.dataset.k); render(); });
action('s-fold', (el) => { const k = el.dataset.k; if (sFold.has(k)) sFold.delete(k); else sFold.add(k); show(); });
paneOf('sessions', show);
action('go-live', () => go('sessions', { live: '1' }));
action('go-apps', () => go('apps'));

// ---------- the pane: the selected session beside the list (docs/project-review-2026-11/mockups/b-sessions.html) ----------
// what the pane last read of the selected session, by the reads its own page uses (`transcript`, `session_stats`):
// `key` is the session's, `gone` a transcript that could not be read
const pn = { key: '', t: null, ss: null, gone: false, busy: false };
const mark = { ok: ['done', '<path d="m5 12 5 5 9-10"/>'], bad: ['failed', '<path d="M6 6l12 12M18 6 6 18"/>'], busy: ['running', '<path d="M12 3a9 9 0 1 0 9 9"/>'] };
/** The mark of a step: a tick, a cross or an open ring, each a shape of its own and named to a screen reader. */
const stepMark = (k) => `<svg class="pn-mark ${k}" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" role="img" aria-label="${mark[k][0]}">${mark[k][1]}</svg>`;

/** The pane of a session, as [its name, the head, the sections]: what the row says at once; what its transcript adds
 * (the steps, the last reply, the files changed) when that is read. A session from another computer shows what the
 * index knows and says where the rest is: its transcript is fetched only when it is opened. */
function paneParts(s, pcs) {
  const key = s.profile + '/' + s.id, mine = pn.key === key, t = mine ? pn.t : null, ss = mine ? pn.ss : null;
  const remote = isRemote(s, pcs), device = s.device_name || nameOf(pcs, s.device) || (S.state && S.state.computer) || 'this computer';
  const tool = TOOL[s.harness] || s.harness, title = s.title || 'Untitled session', hash = sessionHash(s, pcs);
  const project = s.project_name || (s.cwd || '').split(/[\\/]/).filter(Boolean).pop() || 'no folder';
  const steps = t ? t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking') : null;
  const failed = steps ? steps.filter((st) => st.status === 'error').length : s.failed || 0;
  const files = steps ? new Set(steps.filter((st) => st.change).map((st) => st.change.path)).size : null;
  const tokens = t ? t.meta.tokens : s.tokens, endedAt = (t ? t.meta.last_active_at : 0) || s.last_active_at;
  const head = `${toolIcon(s.harness, 28)}<div class="grow"><div class="name trunc" title="${esc(title)}" data-testid="pane-name">${esc(title)}</div>
    <div class="row small muted pn-where"><span class="trunc">${esc(project)}</span><span>·</span>${syncOn(pcs) && s.device ? cdot(pcs, s.device, device, 'on') : ''}<span class="trunc">${esc(device)}</span></div></div>`;
  // the state strip: live (and that the pane follows it) with the time of its last step; or when it ended, and what failed
  const [tone, dot, state, note] = s.archived ? ['plain', '<i class="a-dot"></i>', `archived · ended ${esc(when(endedAt))}`, '']
    : s.open ? ['', '<i class="live-pulse-dot"></i>', '<span class="acT">live · following</span>', `last step ${esc(hm(endedAt))}`]
    : s.open_elsewhere ? ['', '<i class="live-pulse-dot"></i>', `<span class="acT">live on ${esc(device)}</span>`, `last step ${esc(hm(endedAt))}`]
    : ['plain', `<i class="a-dot ${failed ? 'bad' : ''}"></i>`, `ended ${esc(when(endedAt))}`, failed ? `<b class="negT">${failed} failed</b>` : ''];
  // ONE primary for the state, as on the row and on the session's page: archived, read only; running here, follow;
  // from another computer, resume here or why not; else resume
  const primary = s.archived ? '<span class="reason" data-testid="pane-reason">archived: it can be read, not resumed</span>'
    : s.open ? `<a class="btn primary grow" href="${hash}" data-testid="pane-primary">${ic.play}Follow live</a>`
    : remote ? (canResume(s.resumable) ? `<button class="btn primary grow" data-act="resume-here" data-s="${resumeData(s)}" data-testid="pane-primary">Resume here</button>` : `<span class="reason" title="${esc(s.resumable || '')}" data-testid="pane-reason">${esc(reasonShort(s.resumable) || 'on ' + device)}</span>`)
    : `<button class="btn primary grow" data-act="resume" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-cwd="${esc(s.cwd)}" title="Resume in ${esc(termName())}" data-testid="pane-primary">Resume</button>`;
  const model = s.model || (t && (t.turns.findLast((x) => x.model) || {}).model) || '';
  const span = s.open || s.open_elsewhere ? `running ${dur(Date.now() - s.started_at)}` : `ended ${new Date(endedAt).toDateString() === new Date(s.started_at).toDateString() ? hm(endedAt) : when(endedAt)} · ${dur(endedAt - s.started_at)}`;
  const tile = (label, value, cls = '', why = '') => `<div class="pane-tile" ${why ? `title="${esc(why)}"` : ''}><span class="label">${label}</span><b class="${cls}">${value}</b></div>`;
  const unread = remote ? `In the transcript on ${device}` : pn.gone && mine ? 'The transcript could not be read' : 'Reading the transcript…';
  const last5 = (steps || []).slice(-5), reply = t && t.turns.findLast((x) => x.answer && x.answer.length);
  const step = (st) => { const bad = st.status === 'error', busy = st.status === 'running' || st.status === 'pending';
    return `${stepMark(bad ? 'bad' : busy ? 'busy' : 'ok')}<b class="trunc">${esc(st.tool)}</b><span class="mono trunc" title="${esc(st.input)}">${esc(st.input)}</span><span class="small ${bad ? 'negT' : busy ? 'acT' : 'muted'} ${bad || busy ? 'strong' : ''}">${bad ? 'failed' : busy ? 'running' : st.change ? `+${st.change.added} −${st.change.removed}` : ''}</span>`; };
  const rest = remote ? `<div class="pane-sec"><p class="pane-note" data-testid="pane-rest">Its steps and its last reply are in the transcript on ${esc(device)}. Open transcript fetches it to this computer.</p></div>`
    : !t ? `<div class="pane-sec"><p class="pane-note" data-testid="pane-rest">${pn.gone && mine ? 'The transcript could not be read.' : 'Reading the transcript…'}</p></div>`
    : `<div class="pane-sec"><div class="pane-sub"><span>Last steps</span><span>newest last</span></div>${last5.length ? `<div class="pn-steps" data-testid="pane-steps">${last5.map(step).join('')}</div>` : '<p class="pane-note">No step yet.</p>'}</div>
      <div class="pane-sec"><div class="pane-sub"><span>Last reply</span><span>${esc(tool)}${reply && reply.answer_at ? ' · ' + esc(hm(reply.answer_at)) : ''}</span></div>${reply ? `<div class="a-tx pn-bubble" data-testid="pane-reply">${md(reply.answer.at(-1))}</div>` : '<p class="pane-note" data-testid="pane-reply">No reply yet.</p>'}</div>`;
  return [title, head, `<div class="pane-sec">
      <div class="pane-why ${tone}" data-testid="pane-state">${dot}<b>${state}</b><span class="grow"></span><span class="small muted">${note}</span></div>
      <div class="pane-acts">${primary}<a class="btn grow" href="${hash}" ${s.open ? `data-act="pane-transcript" data-key="${esc(key)}"` : ''} data-testid="pane-open">Open transcript</a></div>
      ${remote ? '' : `<div class="pane-acts"><button class="btn grow" data-act="export-md" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-title="${esc(title)}" data-testid="pane-export">Export as Markdown</button><button class="btn icon" data-act="session-menu" data-path="${esc(s.source_path)}" data-profile="${esc(s.profile)}" data-id="${esc(s.id)}" data-cwd="${esc(s.cwd)}" data-resume="${!s.archived && s.open ? 1 : ''}" data-terms="${s.archived ? '' : 1}" aria-label="More" aria-haspopup="menu" data-testid="pane-menu">⋯</button></div>`}</div>
    <div class="pane-sec"><dl class="pane-kv short"><dt>Account</dt><dd class="row" data-testid="pane-account">${toolIcon(s.harness, 14)}<span class="trunc">${esc(acctName(s.profile))}</span></dd><dt>Tool</dt><dd>${esc(tool)}</dd>
      <dt>Model</dt><dd class="mono trunc">${model ? esc(model) : '<span class="dim" title="Its log names no model">—</span>'}</dd><dt>Started</dt><dd>${esc(when(s.started_at))} <span class="muted">· ${esc(span)}</span></dd></dl></div>
    <div class="pane-sec"><div class="pane-tiles">${tile('Tokens', fmt(tokens))}${tile('Read from cache', ss && ss.requests ? Math.round(ss.hit * 100) + '%' : '—', '', ss && ss.requests ? '' : remote || !mine || (!t && !pn.gone) ? unread : 'None of its requests is in the usage records')}${tile('Failed commands', String(failed), failed ? 'negT' : '')}${tile('Files changed', files == null ? '—' : String(files), '', files == null ? unread : '')}</div></div>
    ${rest}`];
}
/** Read what the pane shows of the selected session (not a session of another computer: its transcript stays where
 * it is until it is opened), then draw the pane alone. `again`: its log file changed. */
async function readPane(key, again = false) {
  if (pn.key === key && (pn.busy || (!again && (pn.t || pn.gone)))) return;
  if (pn.key !== key) Object.assign(pn, { key, t: null, ss: null, gone: false, at: null });
  pn.busy = true;
  const i = key.indexOf('/'), who = { profile: key.slice(0, i), id: key.slice(i + 1) };
  const [t, ss] = await Promise.all([call('transcript', who).catch(() => null), call('session_stats', who).catch(() => null)]);
  if (pn.key !== key) return; // another session was selected while this one was read
  Object.assign(pn, { busy: false, t: t || pn.t, ss: ss || pn.ss, gone: !t && !pn.t });
  if (S.route.view !== 'sessions' || selected() !== key || !last) return;
  // a session the list does not hold has nothing but its transcript: the page is drawn with it; else the pane alone
  if (!last.picked) return show();
  patchPane(...paneParts(last.picked, last.pcs));
}
// the selected session is read once, and followed while it runs here: the list beside it is not drawn again
let paneWatch = '';
function followPane(key, s, pcs) {
  if (s && isRemote(s, pcs)) return;
  // read once; again when the list's fresh read says the session went on since (a session that is not watched)
  const moved = !!s && pn.key === key && pn.at != null && pn.at !== s.last_active_at;
  if (s) pn.at = s.last_active_at;
  readPane(key, moved);
  if (!s || !s.open || !s.source_path || paneWatch === key + '|' + s.source_path) return;
  paneWatch = key + '|' + s.source_path;
  watchFile(s, () => { const on = S.route.view === 'sessions' && selected() === key && !!document.getElementById('pane'); if (!on) paneWatch = ''; return on; }, () => readPane(key, true));
}
// Open transcript on a running session: its page from the start, not following (Follow live opens it at its end)
action('pane-transcript', (el) => { Object.assign(rv, { sessionKey: el.dataset.key, open: new Set(), more: new Set(), mode: 'all', follow: false }); location.hash = el.getAttribute('href'); });

// ---------- read a session ----------
// a header, five numbers, and the conversation as a chat with each turn's steps (folded) where they happened;
// Changes and Failed list only those steps. open: the steps unfolded; more: the long runs shown in full;
// follow: stay at the end of the conversation as a live session grows
const rv = { mode: 'all', open: new Set(), more: new Set(), sessionKey: '', follow: false, pending: [] };
const secsOf = (ms) => (ms >= 1000 ? (ms / 1000).toFixed(1) + ' s' : ms + ' ms');
const diffHtml = (d) => `<div class="a-diff mono">${String(d || '').split('\n').map((l) => `<span class="${l[0] === '+' ? 'a-add' : l[0] === '-' ? 'a-del' : l.startsWith('@@') ? 'a-hunk' : ''}">${esc(l) || ' '}</span>`).join('')}</div>`;
/** The terminal a session resumes in, as its button says it. */
const termName = () => { const win = !S.state || !S.state.os || S.state.os === 'windows'; return !win ? 'Terminal' : S.state.settings.terminal === 'console' ? 'a console window' : 'Windows Terminal'; };

view('session', async (sheet, params) => {
  sheet.className = 'sheet';
  const key = params.profile + '/' + params.id;
  // a redraw of the shown session (live update, unfolding a step) keeps it on screen until the new one is ready
  if (rv.sessionKey !== key || !sheet.querySelector('#chat')) sheet.innerHTML = `<div class="page-body">${pageHead('sessions', 'Opening the session…')}</div>`;
  // a session from another computer: its transcript is fetched from the sync folder the first time
  if (params.device && rv.sessionKey !== key) toast('Fetching from the folder…');
  const [t, ss, tl] = await Promise.all([call('transcript', { profile: params.profile, id: params.id, device: params.device }), call('session_stats', { profile: params.profile, id: params.id }).catch(() => null), S.cache.tags ? null : call('tag_list').catch(() => [])]);
  if (tl) S.cache.tags = tl;
  if (!S.state) await loadState();
  const run = !params.device && runOf(params.profile, params.id);
  if (rv.sessionKey !== key) { rv.sessionKey = key; rv.open = new Set(); rv.more = new Set(); rv.follow = !!t.meta.open || !!run; rv.mode = 'all'; }
  read = { t, ss, params };
  S.cache.transcript = t;
  drawSession(sheet);
  // a session hrm run started is running, whatever the consoles say
  watchLive({ ...t.meta, open: t.meta.open || !!run });
});

// what the session page last read: unfolding a step, showing a long run in full or choosing All, Changes or Failed
// only changes what is shown, so it draws from this again and asks the backend nothing
let read = null;
function showSession() {
  const p = S.route.params;
  if (read && S.route.view === 'session' && S.drawn === location.hash && read.params.profile === p.profile && read.params.id === p.id) drawSession(document.getElementById('sheet')); else render();
}
function drawSession(sheet) {
  const { t, ss, params } = read;
  const m = t.meta, pcs = S.cache.computers;
  // the run hrm run started for it (hooks.rs): its request at the end of the conversation, and the reply box under it
  const r = params.device ? null : runOf(m.profile, m.id), live = m.open || !!r;
  // a reply sent is pending until its prompt shows in the transcript
  rv.pending = rv.pending.filter((p) => p.key !== rv.sessionKey || said(t, p.text) <= p.n);
  const pending = rv.pending.filter((p) => p.key === rv.sessionKey).map((p, i) => `<div class="a-msg you pending" data-key="pend:${i}" data-testid="reply-pending"><span class="a-who">YOU · sending…</span><div class="a-tx">${esc(p.text)}</div></div>`).join('');
  const steps = t.turns.flatMap((x) => x.steps).filter((st) => st.kind !== 'thinking');
  const changes = steps.filter((st) => st.change), failed = steps.filter((st) => st.status === 'error'), files = new Set(changes.map((st) => st.change.path)).size;
  const win = !S.state || !S.state.os || S.state.os === 'windows';
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
    : list.slice(0, head).map(([st, k]) => row(st, k)).join('') + `<div class="a-more" data-key="more:${id}"><button class="btn sm" data-act="more-steps" data-run="${id}" data-testid="more-steps">Show ${(list.length - head - tail).toLocaleString()} more steps</button></div>` + list.slice(-tail).map(([st, k]) => row(st, k)).join('')}</div>`;
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
  // The head's actions, on one row: ONE primary (the action that works in the state shown), Export as a secondary,
  // the rest behind ⋯. Archived: read only. From another computer: Resume here, or why not. Running here: Follow live
  // (Resume moves into ⋯). Else: Resume in the terminal of the setting, with its ▾ to change the terminal.
  const local = !m.archived && !params.device;
  const primary = m.archived ? '<span class="muted" data-testid="resume-reason">archived: it can be read, not resumed</span>'
    : params.device ? (canResume(m.resumable) ? `<button class="btn primary" data-act="resume-here" data-s="${resumeData(m)}" data-testid="resume-here">Resume here</button>` : `<span class="muted" title="${esc(m.resumable || '')}" data-testid="resume-reason">${esc(reasonShort(m.resumable))}</span>`)
    : live ? '<button class="btn primary" data-act="follow-end" data-testid="follow-live">Follow live</button>'
    : `<span class="a-split"><button class="btn primary" data-act="resume" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}" data-cwd="${esc(m.cwd)}" data-testid="resume">Resume in ${termName()}</button>${win ? `<button class="btn primary" data-act="terminal-menu" aria-label="Choose terminal" aria-haspopup="menu">▾</button>` : ''}</span>`;
  const stat = (title, value, cls = '') => `<div class="account-stat"><span>${title}</span><b class="${cls}">${value}</b><span></span></div>`;
  patch(sheet, `<div class="page-body a-sbody" data-tour="session">${detailHead({ back: ['#sessions', 'Sessions · ' + project], title: esc(m.title || 'Untitled session'), titleAttrs: 'data-testid="session-title"',
      status: `<div class="a-meta">${live ? `<span class="a-pill live"><i class="live-pulse-dot"></i>LIVE${rv.follow ? ' · following' : ''}</span>` : ''}${m.archived ? '<span class="a-pill arch" data-testid="session-archived">archived</span>' : ''}<span data-testid="session-account">${toolIcon(m.harness, 14)} ${esc(acctName(m.profile))}${!params.device && acctEmail(m.profile) ? ' · ' + esc(acctEmail(m.profile)) : ''} · ${esc(tool)}</span>${tagsHtml(m.tags, 'session', m.profile + '/' + m.id, m.title || 'this session')}${m.model ? `<span>${esc(m.model)}</span>` : ''}
        ${params.device ? `<span data-testid="session-from">${cdot(pcs, params.device, m.device_name, 'on')} ran on ${esc(m.device_name || 'another computer')}</span>` : m.continued_from ? `<span data-testid="session-from">continued from ${esc(m.continued_from)}</span>` : ''}
        <span>started ${esc(when(m.started_at))} · ${plural(t.turns.length, 'turn')}</span><span>${fmt(m.tokens)} tokens${ss && ss.cost != null ? ' · ' + money(ss.cost) + ' ≈' : ''}</span></div>`,
      actions: `${params.device ? '' : `<button class="btn" data-testid="export-md" data-act="export-md" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}">Export as Markdown</button>`}${primary}<button class="btn icon" data-act="session-menu" data-path="${esc(m.source_path)}" data-profile="${esc(m.profile)}" data-id="${esc(m.id)}" data-cwd="${esc(m.cwd)}" data-resume="${local && live ? 1 : ''}" aria-label="More" aria-haspopup="menu" data-testid="session-menu">⋯</button>` })}
    <section class="account-stats" aria-label="Session readouts">${stat('Tokens', fmt(m.tokens))}${stat('Read from cache', ss && ss.requests ? Math.round(ss.hit * 100) + '%' : '—')}${stat('First reply p50', ss && ss.p50 != null ? (ss.p50 / 1000).toFixed(1) + ' s' : '—')}${stat('Failed commands', String(failed.length), failed.length ? 'a-neg' : '')}${stat('Files changed', String(files))}</section>
    <section class="panel-card a-conv" data-testid="conv-panel" data-tour="conversation"><div class="panel-head"><h2 class="panel-title">Conversation<span class="muted">${plural(t.turns.length, 'turn')} · ${steps.length.toLocaleString()} steps · ${files} changed ${files === 1 ? 'file' : 'files'} · ${failed.length} failed</span></h2>
        <span data-tour="modes">${seg('Show', [['all', 'All'], ['changes', 'Changes'], ['failed', 'Failed']], rv.mode, (v) => `data-act="rv-mode" data-v="${v}" data-testid="mode-${v}"`)}</span></div>
      <div class="a-chat" id="chat" data-testid="chat" tabindex="0" aria-label="Conversation; scroll to read">${body || (pending || r ? '' : '<div class="a-empty">This session has no messages yet.</div>')}${pending}${r && r.state === 'waiting' ? askCard(r) : ''}<div class="a-toend" id="toend" hidden><button class="btn" data-act="follow-end" data-testid="follow-end">↓ Latest${live ? ' · follow live' : ''}</button></div></div>${r ? replyBar(r) : ''}</section></div>`);
  // a live session opens at its end, following; a redraw keeps the reader's place (patch), or the end while following
  const chat = document.getElementById('chat');
  if (rv.follow) chat.scrollTop = chat.scrollHeight;
  const atEnd = () => chat.scrollHeight - chat.scrollTop - chat.clientHeight < 40;
  const show = () => { document.getElementById('toend').hidden = atEnd(); };
  chat.onscroll = () => { rv.follow = atEnd(); show(); };
  show();
  const box = document.getElementById('reply-text');
  // one line a message: Enter sends it (a pasted text of several lines is refused by the terminal, which says so)
  if (box) box.onkeydown = (e) => { if (e.key === 'Enter' && !e.isComposing) { e.preventDefault(); sendReply(r.run); } };
}

// ---------- a session that waits for you, and the reply box (the run's hook and its own terminal) ----------
/** How many of the transcript's prompts are this text. */
const said = (t, text) => t.turns.filter((x) => (x.prompt || '').trim() === text).length;
const termBtn = (r, tid, cls = 'btn sm') => `<button class="${cls}" data-act="show-run" data-run="${esc(r.run)}" title="Bring its terminal to the front" data-testid="${tid}">Terminal</button>`;
/** The request the session's hook holds, in full, at the end of the conversation: for a permission the tool, its
 * whole command or file and the tool's description, then Approve once or Deny (once, never "always"); for a question
 * each question with only its own options. The terminal prompt stays answerable too, and the first answer wins. */
function askCard(r) {
  const a = r.ask;
  const head = (title) => `<div class="a-askh"><b class="warnT">${title}</b><span class="grow"></span>${termBtn(r, 'ask-terminal')}</div>`;
  if (!a) return `<section class="a-ask" data-key="ask:none" data-testid="ask-card" aria-label="Waiting for you">${head('Waiting for you')}<p class="small">${esc(r.reason || 'It waits for an answer in its terminal.')}</p></section>`;
  if (a.kind === 'question') {
    return `<section class="a-ask ask" data-key="ask:${esc(a.id)}" data-testid="ask-card" aria-label="A question for you">${head('Asks you')}${(a.input.questions || []).map((q, i) => `<fieldset class="ask-q" data-q="${i}"><legend>${esc(q.question)}</legend>${(q.options || []).map((o) => `<label class="chk"><input type="${q.multiSelect ? 'checkbox' : 'radio'}" name="ask-${esc(a.id)}-${i}" value="${esc(o.label)}"> <span><b>${esc(o.label)}</b>${o.description ? ` <span class="muted">${esc(o.description)}</span>` : ''}</span></label>`).join('')}</fieldset>`).join('')}
      <div class="row-actions"><button class="btn" data-act="ask-send" data-id="${esc(a.id)}" data-testid="ask-send">Send answer</button></div></section>`;
  }
  return `<section class="a-ask ask" data-key="ask:${esc(a.id)}" data-testid="ask-card" aria-label="A permission request">${head(`Wants to use ${esc(a.tool)}`)}<pre class="ask-what mono" tabindex="0" data-testid="ask-what">${esc(a.what || a.tool)}</pre>${a.input.description ? `<p class="small muted" data-testid="ask-desc">${esc(a.input.description)}</p>` : ''}
    <div class="row-actions"><button class="btn" data-act="ask-answer" data-id="${esc(a.id)}" data-behavior="allow" data-testid="ask-allow">Approve once</button><button class="btn" data-act="ask-answer" data-id="${esc(a.id)}" data-behavior="deny" data-testid="ask-deny">Deny</button></div></section>`;
}
async function sendAnswer(id, a, done) {
  const ok = await attempt(() => call('answer', { id, ...a }));
  if (ok === true) toast(done);
  else if (ok === false) toast('That request was answered in the terminal or has expired.');
}
action('ask-answer', (el) => sendAnswer(el.dataset.id, { behavior: el.dataset.behavior }, el.dataset.behavior === 'allow' ? 'Approved once.' : 'Denied.'));
action('ask-send', (el) => {
  const r = (S.needs || []).find((x) => x.ask && x.ask.id === el.dataset.id);
  if (!r) return toast('That request was answered in the terminal or has expired.');
  const box = el.closest('.ask'), answers = {};
  for (const [i, q] of (r.ask.input.questions || []).entries()) {
    const picked = [...box.querySelectorAll(`[data-q="${i}"] input:checked`)].map((x) => x.value);
    if (!picked.length) return toast(`Choose an answer for "${q.question}".`, true);
    answers[q.question] = picked;
  }
  return sendAnswer(r.ask.id, { answers }, 'Answer sent.');
});
/** Under the conversation of a session hrm run started: a line to type into it, Stop (Ctrl+C) and its terminal. Only
 * a run that owns its terminal (the own_terminal setting, when it started) takes them; else the box says why. */
function replyBar(r) {
  const on = r.terminal, dis = on ? '' : 'disabled';
  // only while a turn runs or a prompt waits: at the idle prompt Ctrl+C exits Codex, and asks Claude Code to exit
  const busy = r.state === 'working' || r.state === 'waiting';
  const stopAttr = on && busy ? 'title="Press Ctrl+C in the session"' : `disabled title="${on ? 'Nothing to stop: the session is at its prompt' : 'Needs Harness Manager’s own terminal'}"`;
  return `<div class="a-reply" data-key="reply" data-testid="reply-bar"><div class="a-reply-row">
      <textarea id="reply-text" rows="2" aria-label="Reply to the session" placeholder="${on ? 'Reply to the session · Enter sends one line' : 'Replies need Harness Manager’s own terminal'}" ${dis} data-testid="reply-text"></textarea>
      <div class="a-reply-acts"><button class="btn" data-act="reply-send" data-run="${esc(r.run)}" ${dis} data-testid="reply-send">Send</button><button class="btn" data-act="reply-stop" data-run="${esc(r.run)}" ${stopAttr} data-testid="reply-stop">Stop</button>${termBtn(r, 'reply-terminal', 'btn')}</div></div>
    ${on ? '' : `<p class="a-reply-off small muted" data-testid="reply-off">Replies need Harness Manager’s own terminal; this session started without it.${S.state && S.state.settings.own_terminal ? ' It is on for the next start.' : ' <button class="btn sm" data-act="own-terminal-on" data-testid="own-terminal-on">Turn it on for the next start</button>'}</p>`}</div>`;
}
async function sendReply(run) {
  const box = document.getElementById('reply-text'), text = box ? box.value.trim() : '';
  if (!text) return;
  if ((await attempt(() => call('reply', { run, text }))) !== true) return;
  box.value = '';
  clearNotice(); // an earlier refusal (several lines) is answered
  rv.pending.push({ key: rv.sessionKey, text, n: read ? said(read.t, text) : 0 });
  rv.follow = true;
  showSession();
}
action('reply-send', (el) => sendReply(el.dataset.run));
action('reply-stop', (el) => attempt(() => call('stop', { run: el.dataset.run }), 'Ctrl+C sent to the session.'));
action('own-terminal-on', () => mutate(() => call('settings_set', { key: 'own_terminal', value: true }), 'Harness Manager’s own terminal is on from the next start.'));
// the Overview's line, a row's "needs you" and the message's Show: the session's page at its request; the run's
// console when it has not said which session it is yet
action('open-run', async (el) => {
  const r = (S.needs || []).find((x) => x.run === el.dataset.run);
  if (!r) return toast('That session has ended.');
  if (!r.session_id) { const ok = await attempt(() => call('show_window', { pid: r.pid, hwnd: r.hwnd })); if (ok === false) toast('That session is in another window; switch to it from the taskbar.'); return; }
  rv.follow = true;
  const p = S.route.params;
  if (S.route.view === 'session' && p.profile === r.profile && p.id === r.session_id && !p.device) showSession(); else go('session', { profile: r.profile, id: r.session_id });
});

// an open session redraws while its tool writes to it: the log file's size and time are checked every 2 s.
// One watch at a time: the session on its page, or the one in the Sessions pane (`alive` says whether it is still shown)
let liveTimer;
function watchFile(m, alive, changed) {
  clearInterval(liveTimer);
  if (!m.open || !m.source_path) return;
  let seen = null;
  // the stamp of what was just read, at once: a change in the first 2 s is a change too (a reply's prompt)
  call('file_stamp', { path: m.source_path }).then((s) => { if (seen === null) seen = JSON.stringify(s); }).catch(() => {});
  liveTimer = setInterval(async () => {
    if (!alive()) return clearInterval(liveTimer);
    if (document.visibilityState !== 'visible') return; // minimized: nothing is looked at
    let stamp;
    try { stamp = JSON.stringify(await call('file_stamp', { path: m.source_path })); } catch { return; }
    if (seen === null) { seen = stamp; return; }
    // hidden in the tray, the change is not drawn; it is still a change at the first look once the window shows
    if (stamp === seen || document.getElementById('dialog-root').innerHTML || !(await shown())) return;
    seen = stamp;
    changed();
  }, 2000);
}
export function watchLive(m) {
  const key = rv.sessionKey;
  paneWatch = '';
  watchFile(m, () => S.route.view === 'session' && rv.sessionKey === key, () => render(true));
}
action('follow-end', () => {
  rv.follow = true;
  const chat = document.getElementById('chat');
  if (chat) chat.scrollTop = chat.scrollHeight;
});
action('more-steps', (el) => { rv.more.add(el.dataset.run); showSession(); });
action('step-open', (el) => { const k = +el.dataset.k; if (rv.open.has(k)) rv.open.delete(k); else rv.open.add(k); showSession(); });
action('rv-mode', (el) => { rv.mode = el.dataset.v; showSession(); });
const terminals = () => [
  { label: 'Open sessions in Windows Terminal', run: () => setTerminal('wt') },
  { label: 'Open sessions in a console window', run: () => setTerminal('console') },
];
action('terminal-menu', (el) => openMenu(el, terminals()));
const setTerminal = (v) => mutate(() => call('settings_set', { key: 'terminal', value: v }), 'Saved');
// ⋯: the log file's folder; for a session running here also Resume (the head's primary is Follow live) and the terminal.
// In the Sessions pane, whose Resume has no ▾ beside it, the terminal is chosen here for every session (`terms`)
action('session-menu', (el) => { const d = el.dataset, win = !S.state || !S.state.os || S.state.os === 'windows'; openMenu(el, [
  ...(d.resume ? [{ label: 'Resume in ' + termName(), run: () => attempt(() => call('resume', { profile: d.profile, id: d.id, cwd: d.cwd }), 'Resumed in a new terminal') }] : []), ...((d.resume || d.terms) && win ? terminals() : []),
  { label: 'Open the log file folder', run: () => call('open_folder', { path: d.path.replace(/[\\/][^\\/]*$/, '') }) },
]); });
action('export-md', async (el) => {
  // (the pane's button carries the title of the session it shows; the session's page has its transcript)
  const path = await saveFile(`${(el.dataset.title || (S.cache.transcript && S.cache.transcript.meta.title) || 'session').replace(/[^\w -]+/g, '').slice(0, 60)}.md`);
  if (path) attempt(() => call('export_markdown', { profile: el.dataset.profile, id: el.dataset.id, path }), 'Exported');
});

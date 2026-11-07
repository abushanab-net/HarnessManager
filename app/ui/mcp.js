// MCP servers, as the reference draws them: the finding, the status line, a matrix with a row per server and a
// checkbox column per tool (a tool's hand-made entry shows "left alone" in its cell), the Info dialog, the Add / Edit
// dialog with its Form / JSON switch, Import from apps. The page draws from its last data, so the search and the pager
// never wait on the backend. The helpers at the top are shared with Skills and System prompts.

import { S, call, view, toast, attempt, mutate, changed, openDialog, closeDialog, dlgHead, confirmDialog, openMenu, action, render, shownTools, setNavCounts, notice, clearNotice, patch } from './core.js';
import { esc, ic, toolIcon, dur } from './lib.js';

// ---------- the reference's pieces ----------
export const head = (title, facts, actions = '') => `<header class="page-heading"><div><h1>${title}</h1><div class="heading-facts">${facts}</div></div><div class="page-actions">${actions}</div></header>`;
export const badge = (text, tone = 'neutral', title = '') => `<span class="plan-pill b-${tone}"${title ? ` title="${esc(title)}"` : ''}>${esc(text)}</span>`;
export const empty = (title, text = '') => `<div class="b-empty"><b>${esc(title)}</b>${text ? `<span class="muted">${esc(text)}</span>` : ''}</div>`;
export const panel = (title, body, actions = '', attrs = '') => `<section class="panel-card" ${attrs}><div class="panel-head"><h2 class="panel-title">${title}</h2>${actions}</div>${body}</section>`;
export const field = (id, label, value, { type = 'text', extra = '', testid = '' } = {}) => `<div class="form-field"><label for="${id}">${label}</label><input id="${id}" type="${type}" value="${esc(value)}" ${extra} ${testid ? `data-testid="${testid}"` : ''}></div>`;
export const dlgError = (msg) => (msg ? `<div class="dlg-text err" role="alert" id="dlg-err">${esc(msg)}</div>` : '');
export const dlgFoot = (okLabel, okAct, okTestid = '') => `<div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" data-act="${okAct}" ${okTestid ? `data-testid="${okTestid}"` : ''}>${esc(okLabel)}</button></span></div>`;
/** Previous · "1–25 of 41" · Next · Rows [25 ▾]; clamps st.page to the rows. The actions are `<act>-page` and `<act>-per`. */
export function pagerHtml(n, st, act) {
  const max = Math.max(0, Math.ceil(n / st.per) - 1);
  st.page = Math.min(st.page, max);
  return `<div class="b-pager"><button class="btn-secondary" data-act="${act}-page" data-v="${st.page - 1}" ${st.page === 0 ? 'disabled' : ''} data-testid="${act}-prev">Previous</button><span>${n ? st.page * st.per + 1 : 0}–${Math.min(n, (st.page + 1) * st.per)} of ${n}</span><button class="btn-secondary" data-act="${act}-page" data-v="${st.page + 1}" ${st.page === max ? 'disabled' : ''} data-testid="${act}-next">Next</button><label>Rows <select data-act="${act}-per" aria-label="Rows per page" data-testid="${act}-per">${[25, 50, 100].map((v) => `<option ${st.per === v ? 'selected' : ''}>${v}</option>`).join('')}</select></label></div>`;
}
export const toolHead = (h, n, inner) => `<th class="b-tool-cell" title="${esc(h.label)}: ${n}" data-label="${esc(h.label)}"><span>${toolIcon(h.id, 24)}<span>${esc(h.label)}</span></span>${inner}</th>`;
/** One tool's checkbox in a row; a hand-made entry the tool keeps shows "left alone" under it. */
export const toolCell = (kind, name, h, on, kept = false) => `<td class="b-tool-cell ${kept ? 'b-kept' : ''}"><input type="checkbox" data-act="toggle-target" data-kind="${kind}" data-item="${esc(name)}" data-h="${h.id}" ${on ? 'checked' : ''} aria-label="${esc(name)} for ${esc(h.label)}" data-testid="t-${kind}-${esc(name)}-${h.id}">${kept ? `<span class="b-kept-mark" title="${esc(h.label)} has its own entry of this name, written by hand; it is left alone. Edit and save the server here to take it over.">left alone</span>` : ''}</td>`;
export const findText = () => (document.getElementById('find')?.value || '').trim().toLowerCase();

// ---------- shared with Skills ----------
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

// a tick writes the item into that tool's file; the page then redraws from disk, so a refused tick shows unticked
action('toggle-target', async (el) => {
  const rep = await mutate(() => call('target', { kind: el.dataset.kind, item: el.dataset.item, harness: el.dataset.h, on: el.checked }));
  if (rep) syncReport(rep, el.dataset.item);
});
action('skill-takeover', async (el) => {
  const names = JSON.parse(el.dataset.names || '[]');
  if (!(await confirmDialog('Replace the tools\' copies with links?', `The ${names.length === 1 ? 'copy' : 'copies'} of <b>${esc(names.join(', '))}</b> in the tools' own skill folders ${names.length === 1 ? 'moves' : 'move'} into <span class="mono">.hrm-backup</span> beside ${names.length === 1 ? 'it' : 'them'}, and the shared ${names.length === 1 ? 'copy is' : 'copies are'} linked there instead. Nothing is deleted.`, 'Replace', false))) return;
  clearNotice();
  const r = await mutate(() => call('skill_takeover', { names }));
  if (!r) return;
  toast(r.moved.length ? `Replaced ${r.moved.length} ${r.moved.length === 1 ? 'copy' : 'copies'}; the originals are in .hrm-backup` : 'Nothing to replace: no copy stood where a link should be');
  syncReport(r.sync);
});
action('item-remove', async (el) => {
  if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from every tool Harness Manager wrote it into?`, 'Remove')) syncReport(await mutate(() => call('item_remove', { kind: el.dataset.kind, name: el.dataset.name })));
});

// ---------- the list ----------
const line = (it) => (it.transport === 'stdio' ? [it.command, ...(it.args || [])].join(' ') : it.url || '');
const tools = async () => shownTools(S.state || (await call('state'))).filter((h) => h.caps.mcp);
const onFor = (d, name, h) => d.targets.some((t) => t.kind === 'mcp' && t.item === name && t.harness === h && !t.profile);
const keptIn = (last, name, h) => ((last && last.report) || []).some((r) => r.harness === h && (r.kept || []).includes(name));
const toJson = (s) => (s.transport === 'stdio'
  ? { type: 'stdio', command: s.command || '', args: s.args || [], ...(Object.keys(s.env || {}).length ? { env: s.env } : {}) }
  : { type: s.transport, url: s.url || '', ...(Object.keys(s.headers || {}).length ? { headers: s.headers } : {}) });

const mp = { per: 25, page: 0 };
let cache = null; // the last data drawn: the search, the pager and a tick's redraw use it
function draw() {
  const { d, hs, sync, last } = cache, q = findText();
  const list = d.mcp.filter((it) => !q || (it.name + ' ' + line(it)).toLowerCase().includes(q));
  const written = d.targets.filter((t) => t.kind === 'mcp' && !t.profile && hs.some((h) => h.id === t.harness)).length;
  const left = sync ? (sync.conflicts || 0) + (sync.skipped || 0) : 0, n = d.mcp.length;
  const pg = pagerHtml(list.length, mp, 'mcp');
  const rows = list.slice(mp.page * mp.per, (mp.page + 1) * mp.per).map((it) => `<tr data-key="mcp-${esc(it.name)}" data-testid="mcp-row-${esc(it.name)}">
      <td><div class="b-name"><b>${esc(it.name)}</b>${badge(it.transport)}${hs.some((h) => onFor(d, it.name, h.id)) ? '' : badge('Off', 'neutral', 'Not written to any tool')}<button class="icon-btn xs" data-act="mcp-info" data-name="${esc(it.name)}" aria-label="About ${esc(it.name)}" title="Where it is written" data-testid="mcp-info-${esc(it.name)}">${ic.info}</button></div>
        <div class="mono b-command" title="${esc(line(it))}">${esc(line(it))}</div>${it.description ? `<div class="muted b-clamp" title="${esc(it.description)}">${esc(it.description)}</div>` : ''}</td>
      ${hs.map((h) => toolCell('mcp', it.name, h, onFor(d, it.name, h.id), keptIn(last, it.name, h.id))).join('')}
      <td class="b-actions-cell"><button class="btn-secondary" data-act="mcp-edit" data-name="${esc(it.name)}" aria-label="Edit ${esc(it.name)}" data-testid="mcp-edit-${esc(it.name)}">Edit</button> <button class="icon-btn" data-act="mcp-menu" data-name="${esc(it.name)}" aria-label="More for ${esc(it.name)}" aria-haspopup="menu">${ic.dots}</button></td></tr>`).join('');
  const table = `<div class="b-matrix-wrap"><table class="replies-table b-matrix" data-testid="mcp-matrix"><thead><tr><th>Server</th>${hs.map((h) => { const k = d.mcp.filter((it) => keptIn(last, it.name, h.id)).length; return toolHead(h, d.mcp.filter((it) => onFor(d, it.name, h.id)).length, `<small>${d.mcp.filter((it) => onFor(d, it.name, h.id)).length}${k ? ` · ${k} left alone` : ''}</small>`); }).join('')}<th><span class="b-sr">Actions</span></th></tr></thead>
    <tbody>${rows || `<tr><td colspan="${hs.length + 2}">${empty('No server matches the search.')}</td></tr>`}</tbody></table></div>${pg}`;
  patch(document.getElementById('sheet'), `<div class="page-body b-page" data-b-page="mcp">
    ${head(n ? `${n} ${n === 1 ? 'server' : 'servers'} shared. ${left ? `${left} ${left === 1 ? 'entry' : 'entries'} left alone.` : 'Every entry is in step.'}` : 'No MCP servers yet.',
    `${n ? written : 0} ${written === 1 ? 'entry' : 'entries'} written · ${hs.length} tools · ${sync ? 'checked ' + dur(Date.now() - sync.at) + ' ago' : 'not synced yet'}`,
    `<button class="btn-secondary" data-act="mcp-import-all" data-testid="mcp-import">Import from apps</button><button class="btn-primary" data-act="mcp-new" data-testid="mcp-add">Add MCP server</button>`)}
    ${n ? `<div class="b-statusline">${badge(`${left} left alone`, left ? 'warn' : 'good')}<span class="muted">${left ? 'Written by hand in a tool\'s file, so not managed here: edit and save the server to take it over.' : 'Handwritten entries stay until you take them over.'}</span></div>${panel('Write to each tool', table, pg)}`
    : panel('Servers', empty('Add a server, or import from your tools.'))}</div>`);
}
async function mcpView(sheet) {
  sheet.className = 'sheet';
  const [d, sync, last, hs] = await Promise.all([call('items'), call('sync_last').catch(() => null), call('mcp_last_import').catch(() => null), tools()]);
  setNavCounts({ 'n-mcp': d.mcp.length });
  cache = { d, hs, sync, last };
  S.pageFind = () => { mp.page = 0; draw(); };
  draw();
}
view('mcp', mcpView);
// the old add / edit route: the list, with the dialog open
view('mcp-edit', async (sheet, params) => { history.replaceState(null, '', '#mcp'); await mcpView(sheet); editMcp(params.name || ''); });
action('mcp-page', (el) => { mp.page = +el.dataset.v; draw(); document.querySelector('.b-matrix-wrap')?.scrollTo(0, 0); });
action('mcp-per', (el) => { mp.per = +el.value; mp.page = 0; draw(); });

/** About one server: its connection, and in each tool's file whether it is written, not written, or left alone. */
action('mcp-info', async (el) => {
  const name = el.dataset.name;
  const [d, last, hs] = await Promise.all([call('items'), call('mcp_last_import').catch(() => null), tools()]);
  const it = d.mcp.find((m) => m.name === name);
  if (!it) return;
  const report = (last && last.report) || [];
  openDialog(`${dlgHead(name)}<div class="b-dialog-content" data-testid="mcp-info">
    <b>Connection · ${esc(it.transport)}</b><div class="mono b-command">${esc(line(it))}</div>
    ${it.description ? `<span>${esc(it.description)}</span>` : ''}${it.homepage ? `<a href="#" data-act="app-site" data-url="${esc(it.homepage)}">Homepage / docs</a>` : ''}
    <h3 class="panel-title">In each tool</h3>
    ${hs.map((h) => { const r = report.find((x) => x.harness === h.id), kept = !!(r && (r.kept || []).includes(name));
      const on = onFor(d, name, h.id) || (r && r.profile && d.targets.some((t) => t.kind === 'mcp' && t.item === name && t.harness === h.id && t.profile === r.profile));
      return `<div class="account-session"><div><div class="b-name">${toolIcon(h.id, 16)}<b>${esc(h.label)}</b>${badge(kept ? 'Left alone' : on ? 'Written' : 'Not written', kept ? 'warn' : on ? 'good' : 'neutral')}</div>
        <div class="mono">${r && r.file ? `${esc(r.file)} ${r.exists === false ? '(not there yet)' : '(' + esc(r.part || '') + ')'}` : 'not read yet'}</div>
        ${kept ? '<span class="muted">Handwritten entry: edit and save here to take it over, or remove the tool\'s own entry.</span>' : ''}</div></div>`; }).join('')}
  </div><div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Close</button><button class="btn-primary" data-act="mcp-edit" data-name="${esc(name)}">Edit</button></span></div>`, { label: name });
});

action('mcp-new', () => editMcp(''));
action('mcp-edit', (el) => editMcp(el.dataset.name));
action('mcp-menu', (el) => openMenu(el, [
  { label: 'Copy as JSON', run: async () => {
    const it = (await call('items')).mcp.find((m) => m.name === el.dataset.name);
    await navigator.clipboard.writeText(JSON.stringify({ [it.name]: toJson(it) }, null, 2));
    toast('Copied');
  } },
  { label: 'Remove…', danger: true, run: async () => {
    if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from Harness Manager and from every tool it wrote it into? Handwritten entries stay.`, 'Remove')) {
      syncReport(await mutate(() => call('item_remove', { kind: 'mcp', name: el.dataset.name })));
    }
  } },
]));

action('mcp-import-all', async () => {
  const r = await attempt(() => call('mcp_import_all'));
  if (!r) return;
  const n = new Set(r.flatMap((x) => x.new || [])).size;
  toast(n ? `Imported ${n} new ${n === 1 ? 'server' : 'servers'}` : 'No new MCP servers found');
  changed();
});

// ---------- add / edit: the reference's dialog; the draft is kept here, so adding a row or switching the mode keeps what was typed ----------
let fd = null, card = null;
const pairs = (rows) => Object.fromEntries(rows.filter(([k]) => k.trim()).map(([k, v]) => [k.trim(), v]));
function server() {
  const stdio = fd.transport === 'stdio';
  return { name: fd.name.trim(), transport: fd.transport, command: stdio ? fd.command.trim() : null, args: stdio ? fd.args.filter((a) => a !== '') : [],
    env: stdio ? pairs(fd.env) : {}, url: stdio ? null : fd.url.trim(), headers: stdio ? {} : pairs(fd.headers),
    description: fd.description.trim() || null, homepage: fd.homepage.trim() || null };
}
function fromJson(text) {
  const j = JSON.parse(text);
  if (!j || Array.isArray(j) || typeof j !== 'object') throw new Error('expected a server object');
  const t = j.type || (j.url ? 'http' : 'stdio');
  if (!['stdio', 'http', 'sse'].includes(t)) throw new Error('type must be stdio, http or sse');
  for (const k of ['command', 'url']) if (j[k] != null && typeof j[k] !== 'string') throw new Error(k + ' must be a string');
  if (j.args && (!Array.isArray(j.args) || j.args.some((x) => typeof x !== 'string'))) throw new Error('args must be an array of strings');
  for (const k of ['env', 'headers']) if (j[k] && (Array.isArray(j[k]) || typeof j[k] !== 'object' || Object.values(j[k]).some((x) => typeof x !== 'string'))) throw new Error(k + ' must contain string values');
  Object.assign(fd, { transport: t, command: j.command || '', args: j.args || [], url: j.url || '', env: Object.entries(j.env || {}), headers: Object.entries(j.headers || {}) });
}
const validUrl = (u) => /^https?:\/\/\S+$/i.test(u);
const kvRows = (list, kind) => `${list.map(([k, v], i) => `<div class="b-kv" data-key="${kind}-${i}">${field(`mc-${kind}-${i}-k`, kind === 'env' ? 'NAME' : 'Header', k, { extra: `data-kv="${kind}" data-i="${i}" data-p="0" class="mono"` })}${field(`mc-${kind}-${i}-v`, 'Value', v, { type: 'password', extra: `data-kv="${kind}" data-i="${i}" data-p="1" class="mono"` })}<button class="btn-secondary" data-act="mc-peek" aria-label="Show or hide the value">Show</button><button class="btn-secondary" data-act="mc-del" data-list="${kind}" data-i="${i}" aria-label="Remove">Remove</button></div>`).join('')}
  <div><button class="btn-secondary" data-act="mc-addrow" data-list="${kind}" data-testid="mc-add-${kind}">+ Add ${kind === 'env' ? 'variable' : 'header'}</button></div>`;
function formHtml(hs) {
  const stdio = fd.transport === 'stdio';
  return `${dlgHead(fd.editing ? 'Edit MCP server' : 'Add MCP server')}<div class="b-dialog-content" data-testid="mcp-form">${dlgError(fd.error)}
    ${field('mc-name', 'Name *', fd.name, { extra: `class="mono" data-f="name" ${fd.editing ? 'readonly title="The name is the key in every tool\'s config"' : 'placeholder="github"'}`, testid: 'mc-name' })}
    <div class="b-toolbar"><b>Connection</b><div class="chip-group" role="group" aria-label="Connection as"><button class="filter-chip ${fd.mode === 'form' ? 'active' : ''}" data-act="mc-mode" data-v="form" data-testid="mc-mode-form">Form</button><button class="filter-chip ${fd.mode === 'json' ? 'active' : ''}" data-act="mc-mode" data-v="json" data-testid="mc-mode-json">JSON</button></div></div>
    ${fd.mode === 'json' ? `<div class="form-field"><label for="mc-json">Claude Code server JSON</label><textarea id="mc-json" class="mono" rows="12" spellcheck="false" data-f="json" data-testid="mc-json">${esc(fd.json)}</textarea><span class="muted">The server as Claude Code writes it: type, then command, args and env, or url and headers.</span></div>`
    : `<div class="form-field"><label for="mc-transport">Transport</label><select id="mc-transport" data-act="mc-transport" data-testid="mc-transport">${[['stdio', 'Local command (stdio)'], ['http', 'HTTP'], ['sse', 'SSE']].map(([k, l]) => `<option value="${k}" ${fd.transport === k ? 'selected' : ''}>${l}</option>`).join('')}</select></div>
      ${stdio ? `${field('mc-cmd', 'Command *', fd.command, { extra: 'class="mono" placeholder="npx" data-f="command"', testid: 'mc-cmd' })}<b>Arguments</b>
        ${fd.args.map((a, i) => `<div class="b-kv" data-key="arg-${i}">${field(`mc-arg-${i}`, `Argument ${i + 1}`, a, { extra: `class="mono" data-arg="${i}"`, testid: `mc-arg-${i}` })}<button class="btn-secondary" data-act="mc-del" data-list="args" data-i="${i}" aria-label="Remove argument ${i + 1}">Remove</button></div>`).join('')}
        <div><button class="btn-secondary" data-act="mc-addrow" data-list="args" data-testid="mc-add-arg">+ Add argument</button></div><b>Environment variables</b>${kvRows(fd.env, 'env')}`
      : `${field('mc-url', 'URL *', fd.url, { extra: 'class="mono" placeholder="https://api.githubcopilot.com/mcp/" data-f="url"', testid: 'mc-url' })}<b>Headers</b>${kvRows(fd.headers, 'headers')}`}`}
    <b>Write to</b><div class="b-toolbar">${hs.map((h) => `<label class="filter-chip"><input type="checkbox" data-wto="${h.id}" ${fd.targets.has(h.id) ? 'checked' : ''} data-testid="mc-to-${h.id}"> ${toolIcon(h.id, 14)} ${esc(h.label)}${h.installed ? '' : ' <span class="muted">(not installed)</span>'}</label>`).join('')}</div>
    <details ${fd.more ? 'open' : ''} id="mc-more"><summary>Description &amp; links · never written to a tool</summary>${field('mc-desc', 'Description', fd.description, { extra: 'data-f="description"' })}${field('mc-home', 'Homepage or docs', fd.homepage, { extra: 'placeholder="https://" data-f="homepage"' })}</details>
  </div>${dlgFoot(fd.editing ? 'Save' : 'Add server', 'mc-save', 'mc-save')}`;
}
async function editMcp(name) {
  const [d, hs] = await Promise.all([call('items'), tools()]);
  const it = d.mcp.find((m) => m.name === name);
  fd = it
    ? { editing: true, mode: 'form', name: it.name, transport: it.transport, command: it.command || '', args: [...(it.args || [])], url: it.url || '',
      env: Object.entries(it.env || {}), headers: Object.entries(it.headers || {}), targets: new Set(hs.filter((h) => onFor(d, it.name, h.id)).map((h) => h.id)),
      description: it.description || '', homepage: it.homepage || '', more: !!(it.description || it.homepage), hs, names: d.mcp.map((m) => m.name) }
    // a new server is written to Claude Code until other tools are ticked
    : { editing: false, mode: 'form', name: '', transport: 'stdio', command: '', args: [], url: '', env: [], headers: [], targets: new Set(hs.filter((h) => h.id === 'claude' && h.installed).map((h) => h.id)), description: '', homepage: '', more: false, hs, names: d.mcp.map((m) => m.name) };
  card = openDialog(formHtml(hs), { size: 'mcp-dlg', label: fd.editing ? 'Edit MCP server' : 'Add MCP server', onClose: () => { fd = null; card = null; } });
}
const redraw = () => { if (fd && card) patch(card, formHtml(fd.hs)); };
// typing goes into the draft; the dialog is only redrawn when a row is added or removed, or the transport or mode changes
document.addEventListener('input', (e) => {
  const el = e.target;
  if (!fd || !card || !card.contains(el)) return;
  if (el.dataset.f) fd[el.dataset.f] = el.value;
  else if (el.dataset.arg) fd.args[+el.dataset.arg] = el.value;
  else if (el.dataset.kv) fd[el.dataset.kv][+el.dataset.i][+el.dataset.p] = el.value;
});
document.addEventListener('change', (e) => {
  const el = e.target;
  if (!fd || !card || !card.contains(el)) return;
  if (el.dataset.wto) { if (el.checked) fd.targets.add(el.dataset.wto); else fd.targets.delete(el.dataset.wto); }
});
document.addEventListener('toggle', (e) => { if (fd && e.target.id === 'mc-more') fd.more = e.target.open; }, true);
// leaving JSON mode takes its text into the draft, or says what is wrong with it
function takeJson() {
  if (fd.mode !== 'json') return true;
  try { fromJson(fd.json); fd.error = ''; return true; } catch (e) { fd.error = 'The JSON is not a server: ' + e.message; redraw(); return false; }
}
action('mc-mode', (el) => { if (el.dataset.v === fd.mode || !takeJson()) return; if (el.dataset.v === 'json') fd.json = JSON.stringify(toJson(server()), null, 2); fd.mode = el.dataset.v; redraw(); });
action('mc-transport', (el) => { fd.transport = el.value; redraw(); });
action('mc-addrow', (el) => { const l = el.dataset.list; fd[l].push(l === 'args' ? '' : ['', '']); redraw(); });
action('mc-del', (el) => { fd[el.dataset.list].splice(+el.dataset.i, 1); redraw(); });
action('mc-peek', (el) => { const v = el.closest('.b-kv').querySelector('input[type=password], input[data-p="1"]'); v.type = v.type === 'password' ? 'text' : 'password'; el.textContent = v.type === 'password' ? 'Show' : 'Hide'; });
action('mc-save', async () => {
  if (!takeJson()) return;
  const s = server();
  const err = !s.name ? 'Give the server a name' : !/^[\w.-]+$/.test(s.name) ? 'Give the server a name using letters, digits, dots, dashes or underscores.'
    : !fd.editing && fd.names.includes(s.name) ? 'That server name already exists.' : s.transport === 'stdio' && !s.command ? 'Enter a command.'
    : s.transport !== 'stdio' && !validUrl(s.url) ? 'Enter an HTTP or HTTPS URL.' : s.homepage && !validUrl(s.homepage) ? 'The homepage needs an HTTP or HTTPS URL.' : '';
  if (err) { fd.error = err; redraw(); return; }
  let rep;
  try { rep = await call('mcp_save', { server: s, replace: fd.editing, targets: [...fd.targets] }); } catch (e) { fd.error = String(e).replace(/^Error: /, ''); redraw(); return; }
  closeDialog();
  syncReport(rep, s.name);
  changed();
});

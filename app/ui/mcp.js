// MCP servers: one list for every tool (search, a tick per tool, edit), an add/edit page with a form or JSON,
// and Import from apps, which reads every tool's config and says which files it read.

import { S, call, view, go, toast, attempt, openDialog, dlgHead, confirmDialog, openMenu, action, render, shownTools, setNavCounts } from './core.js';
import { esc, ic, toolIcon, dur } from './lib.js';
import { syncReport } from './views.js';

const line = (it) => (it.transport === 'stdio' ? [it.command, ...(it.args || [])].join(' ') : it.url || '');
const tools = async () => shownTools(S.state || (await call('state'))).filter((h) => h.caps.mcp);
const onFor = (d, name, h) => d.targets.some((t) => t.kind === 'mcp' && t.item === name && t.harness === h && !t.profile);

view('mcp', async (sheet) => {
  sheet.className = 'sheet';
  const [d, last, sync] = await Promise.all([call('items'), call('mcp_last_import').catch(() => null), call('sync_last').catch(() => null)]);
  const hs = await tools();
  setNavCounts({ 'n-mcp': d.mcp.length });
  const rows = d.mcp.map((it) => {
    const off = !hs.some((h) => onFor(d, it.name, h.id));
    return `<tr data-search="${esc((it.name + ' ' + line(it)).toLowerCase())}" data-testid="mcp-row-${esc(it.name)}">
      <td><div class="mn"><b>${esc(it.name)}</b><span class="tag">${esc(it.transport)}</span>${off ? '<span class="tag off" title="Not written to any tool">Off</span>' : ''}</div>
        <div class="mono mc" title="${esc(line(it))}">${esc(line(it))}</div>${it.description ? `<div class="md">${esc(it.description)}</div>` : ''}</td>
      ${hs.map((h) => `<td class="c"><input type="checkbox" data-act="toggle-target" data-kind="mcp" data-item="${esc(it.name)}" data-h="${h.id}" ${onFor(d, it.name, h.id) ? 'checked' : ''} aria-label="${esc(it.name)} for ${esc(h.label)}" data-testid="t-mcp-${esc(it.name)}-${h.id}"></td>`).join('')}
      <td class="num" style="white-space:nowrap"><button class="iconbtn sq" data-act="mcp-edit" data-name="${esc(it.name)}" aria-label="Edit ${esc(it.name)}" title="Edit" data-testid="mcp-edit-${esc(it.name)}">${ic.edit}</button> <button class="iconbtn sq" data-act="mcp-menu" data-name="${esc(it.name)}" aria-label="More for ${esc(it.name)}">${ic.dots}</button></td></tr>`;
  }).join('');
  const written = d.targets.filter((t) => t.kind === 'mcp' && !t.profile && hs.some((h) => h.id === t.harness)).length;
  const inStep = sync && !(sync.conflicts + sync.skipped);
  sheet.innerHTML = `<h1>MCP servers</h1><p class="find">Kept once here and written into every tool you tick, in that tool's own format. Only entries Harness Manager wrote are ever removed.</p>
    <div class="acts lead"><button class="btn" data-act="mcp-import-all" data-testid="mcp-import">Import from apps</button><button class="btn primary" data-act="mcp-new" data-testid="mcp-add">Add MCP server</button></div>
    <div class="sline">${sync ? `<span class="pill ${inStep ? 'teal' : 'warn'} mono">${inStep ? 'in step' : `${sync.conflicts + sync.skipped} left alone`} · checked ${dur(Date.now() - sync.at)} ago</span>` : '<span class="pill mono">not synced yet</span>'}
      <span class="muted">${d.mcp.length} ${d.mcp.length === 1 ? 'server' : 'servers'} · ${hs.length} tools · ${written} ${written === 1 ? 'entry' : 'entries'} written</span></div>
    <div class="mcpcols"><div class="mcpmain">
    ${d.mcp.length ? `<table class="grid mlist" data-testid="mcp-matrix"><thead><tr><th>Server</th>${hs.map((h) => `<th class="c" title="${esc(h.label)}: ${d.mcp.filter((it) => onFor(d, it.name, h.id)).length} servers" data-label="${esc(h.label)}">${esc(h.label)}</th>`).join('')}<th></th></tr></thead>
      <tbody>${rows}<tr class="nomatch" hidden><td colspan="${hs.length + 2}" class="empty">No server matches the search.</td></tr></tbody></table>`
    : '<p class="empty">No MCP servers yet. Add one, or import the ones your tools already have.</p>'}</div>
    ${last && !hideImport ? importPanel(last) : ''}</div>`;
  const filter = (v) => {
    const q = (v || '').trim().toLowerCase();
    let shown = 0;
    sheet.querySelectorAll('tr[data-search]').forEach((tr) => { tr.hidden = !!q && !tr.dataset.search.includes(q); if (!tr.hidden) shown++; });
    const nm = sheet.querySelector('tr.nomatch');
    if (nm) nm.hidden = shown > 0;
  };
  S.pageFind = filter;
  filter(document.getElementById('find').value);
});

action('mcp-new', () => go('mcp-edit'));
action('mcp-edit', (el) => go('mcp-edit', { name: el.dataset.name }));
action('mcp-menu', (el) => openMenu(el, [
  { label: 'Copy as JSON', run: async () => {
    const it = (await call('items')).mcp.find((m) => m.name === el.dataset.name);
    await navigator.clipboard.writeText(JSON.stringify({ [it.name]: toJson(it) }, null, 2));
    toast('Copied');
  } },
  { label: 'Remove…', danger: true, run: async () => {
    if (await confirmDialog('Remove ' + el.dataset.name, `Remove <b>${esc(el.dataset.name)}</b> from Harness Manager and from every tool it wrote it into?`, 'Remove')) {
      attempt(() => call('item_remove', { kind: 'mcp', name: el.dataset.name })).then((rep) => { syncReport(rep); render(); });
    }
  } },
]));

/** The last Import from apps: what it found and which file of each tool it read. */
let hideImport = false;
function importPanel({ at, report: r }) {
  const found = [...new Set(r.flatMap((x) => x.new || []))], kept = [...new Set(r.flatMap((x) => x.kept || []))];
  return `<aside class="panel mcpside" data-testid="import-report"><div class="ph"><h2>Last import from apps</h2><button class="iconbtn" data-act="mcp-import-hide" aria-label="Hide" title="Hide" data-testid="import-ok">${ic.x}</button></div>
    <p class="muted sm mono">${esc(new Date(at).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' }))}</p>
    <p><b>${found.length ? `Found ${found.length} new ${found.length === 1 ? 'server' : 'servers'}: ${esc(found.join(', '))}.` : 'No new MCP servers found.'}</b></p>
    <p class="muted sm">For servers already in the list, only the matching tool's tick was turned on; the copy in Harness Manager still wins.${kept.length ? ` Left alone, because a different server has that name here: ${esc(kept.join(', '))}.` : ''}</p>
    <div class="cap">Which files were read</div>
    <dl class="ireport">${r.map((x) => `<dt>${toolIcon(x.harness, 14)} ${esc(x.label)}${(x.new || []).length ? ` <b class="acT">+${x.new.length}</b>` : ''}</dt><dd><span class="mono">${esc(x.file || 'no MCP file')}</span> <span class="muted">${x.exists === false ? '(not there yet)' : '(' + esc(x.part || '') + ')'}</span>${x.error ? ` <span class="err">${esc(x.error)}</span>` : ''}</dd>`).join('')}</dl></aside>`;
}
action('mcp-import-all', async () => {
  const r = await attempt(() => call('mcp_import_all'));
  if (!r) return;
  const n = new Set(r.flatMap((x) => x.new || [])).size;
  toast(n ? `Imported ${n} new ${n === 1 ? 'server' : 'servers'}` : 'No new MCP servers found');
  hideImport = false;
  render();
});
action('mcp-import-hide', () => { hideImport = true; render(); });

// ---------- add / edit ----------
// the page's draft; rows are [key, value] pairs so a half-typed entry is kept
let fd = null;
const toJson = (s) => (s.transport === 'stdio'
  ? { type: 'stdio', command: s.command || '', args: s.args || [], ...(Object.keys(s.env || {}).length ? { env: s.env } : {}) }
  : { type: s.transport, url: s.url || '', ...(Object.keys(s.headers || {}).length ? { headers: s.headers } : {}) });
const pairs = (rows) => Object.fromEntries(rows.filter(([k]) => k.trim()).map(([k, v]) => [k.trim(), v]));
function server() {
  const stdio = fd.transport === 'stdio';
  return { name: fd.name.trim(), transport: fd.transport, command: stdio ? fd.command.trim() : null, args: stdio ? fd.args.filter((a) => a !== '') : [],
    env: stdio ? pairs(fd.env) : {}, url: stdio ? null : fd.url.trim(), headers: stdio ? {} : pairs(fd.headers),
    description: fd.description.trim() || null, homepage: fd.homepage.trim() || null };
}
function fromJson(text) {
  const v = JSON.parse(text);
  const t = v.type || (v.url ? 'http' : 'stdio');
  if (!['stdio', 'http', 'sse'].includes(t)) throw new Error('type must be stdio, http or sse');
  Object.assign(fd, { transport: t, command: v.command || '', args: (v.args || []).map(String), url: v.url || '',
    env: Object.entries(v.env || {}).map(([k, x]) => [k, String(x)]), headers: Object.entries(v.headers || {}).map(([k, x]) => [k, String(x)]) });
}

view('mcp-edit', async (sheet, params) => {
  sheet.className = 'sheet';
  const d = await call('items');
  const hs = await tools();
  const key = params.name || '';
  if (!fd || fd.key !== key) {
    const it = d.mcp.find((m) => m.name === key);
    fd = it
      ? { key, editing: true, mode: 'form', name: it.name, transport: it.transport, command: it.command || '', args: [...(it.args || [])], url: it.url || '',
        env: Object.entries(it.env || {}), headers: Object.entries(it.headers || {}), targets: new Set(hs.filter((h) => onFor(d, it.name, h.id)).map((h) => h.id)),
        description: it.description || '', homepage: it.homepage || '', more: !!(it.description || it.homepage) }
      // a new server is written to Claude Code until other tools are ticked
      : { key, editing: false, mode: 'form', name: '', transport: 'stdio', command: '', args: [], url: '', env: [], headers: [], targets: new Set(hs.filter((h) => h.id === 'claude' && h.installed).map((h) => h.id)), description: '', homepage: '', more: false };
  }
  const kv = (list, kind) => `${list.map(([k, v], i) => `<div class="kvrow"><input class="input mono" value="${esc(k)}" placeholder="${kind === 'env' ? 'NAME' : 'Header'}" data-kv="${kind}" data-i="${i}" data-p="0" aria-label="${kind === 'env' ? 'Variable name' : 'Header name'}">
      <input class="input mono" type="password" value="${esc(v)}" placeholder="value" data-kv="${kind}" data-i="${i}" data-p="1" aria-label="Value">
      <button class="iconbtn" data-act="mc-peek" title="Show or hide the value" aria-label="Show the value">${ic.eye}</button><button class="iconbtn" data-act="mc-del" data-list="${kind}" data-i="${i}" aria-label="Remove">${ic.x}</button></div>`).join('')}
    <button class="btn sm linkish" data-act="mc-addrow" data-list="${kind}" data-testid="mc-add-${kind}">${ic.plus} Add ${kind === 'env' ? 'variable' : 'header'}</button>`;
  const stdio = fd.transport === 'stdio';
  sheet.innerHTML = `<div class="rh"><a class="iconbtn" href="#mcp" aria-label="Back to MCP servers">${ic.back}</a><h1>${fd.editing ? 'Edit MCP server' : 'Add MCP server'}</h1></div>
    <div class="mform" data-testid="mcp-form">
      <label class="fl" for="mc-name">Name <span class="req">*</span></label>
      <input class="input mono" id="mc-name" value="${esc(fd.name)}" ${fd.editing ? 'readonly title="The name is the key in every tool\'s config"' : 'placeholder="github"'} data-f="name" data-testid="mc-name">
      <div class="fh"><span class="fl">Connection</span><span class="seg3 sm"><a class="${fd.mode === 'form' ? 'on' : ''}" data-act="mc-mode" data-v="form" data-testid="mc-mode-form">Form</a><a class="${fd.mode === 'json' ? 'on' : ''}" data-act="mc-mode" data-v="json" data-testid="mc-mode-json">JSON</a></span></div>
      ${fd.mode === 'json' ? `<textarea class="input mono" id="mc-json" rows="12" spellcheck="false" data-testid="mc-json">${esc(JSON.stringify(toJson(server()), null, 2))}</textarea>
        <span class="hint">The server as Claude Code writes it: type, then command, args and env, or url and headers.</span>` : `
      <span class="fl">Transport</span>
      <span class="seg3">${[['stdio', 'Local command (stdio)'], ['http', 'HTTP'], ['sse', 'SSE']].map(([k, l]) => `<a class="${fd.transport === k ? 'on' : ''}" data-act="mc-transport" data-v="${k}" data-testid="mc-t-${k}">${l}</a>`).join('')}</span>
      ${stdio ? `<label class="fl" for="mc-cmd">Command <span class="req">*</span></label>
        <input class="input mono" id="mc-cmd" value="${esc(fd.command)}" placeholder="npx" data-f="command" data-testid="mc-cmd">
        <span class="fl">Arguments</span>
        <div class="rows">${fd.args.map((a, i) => `<div class="kvrow one"><input class="input mono" value="${esc(a)}" data-arg="${i}" aria-label="Argument ${i + 1}" data-testid="mc-arg-${i}"><button class="iconbtn" data-act="mc-del" data-list="args" data-i="${i}" aria-label="Remove argument">${ic.x}</button></div>`).join('')}
          <button class="btn sm linkish" data-act="mc-addrow" data-list="args" data-testid="mc-add-arg">${ic.plus} Add argument</button></div>
        <span class="fl">Environment variables</span><div class="rows">${kv(fd.env, 'env')}</div>`
      : `<label class="fl" for="mc-url">URL <span class="req">*</span></label>
        <input class="input mono" id="mc-url" value="${esc(fd.url)}" placeholder="https://api.githubcopilot.com/mcp/" data-f="url" data-testid="mc-url">
        <span class="fl">Headers</span><div class="rows">${kv(fd.headers, 'headers')}</div>`}`}
      <span class="fl">Write to</span>
      <div class="wto">${hs.map((h) => `<label class="switch"><input type="checkbox" data-wto="${h.id}" ${fd.targets.has(h.id) ? 'checked' : ''} data-testid="mc-to-${h.id}"> ${toolIcon(h.id, 16)} ${esc(h.label)}${h.installed ? '' : ' <span class="muted">(not installed)</span>'}</label>`).join('')}</div>
      <details ${fd.more ? 'open' : ''} id="mc-more"><summary><b>Description &amp; links</b> <span class="muted">for you only; never written to a tool</span></summary>
        <div class="mform inner"><label class="fl" for="mc-desc">Description</label><input class="input" id="mc-desc" value="${esc(fd.description)}" data-f="description">
        <label class="fl" for="mc-home">Homepage or docs</label><input class="input" id="mc-home" value="${esc(fd.homepage)}" placeholder="https://" data-f="homepage"></div></details>
    </div>
    <div class="formfoot"><button class="btn" data-act="mc-cancel">Cancel</button><button class="btn primary" data-act="mc-save" data-testid="mc-save">${fd.editing ? 'Save' : 'Add server'}</button></div>`;
  // typing goes into the draft; only adding or removing a row, or switching transport or mode, redraws
  sheet.querySelectorAll('[data-f]').forEach((el) => (el.oninput = () => { fd[el.dataset.f] = el.value; }));
  sheet.querySelectorAll('[data-arg]').forEach((el) => (el.oninput = () => { fd.args[+el.dataset.arg] = el.value; }));
  sheet.querySelectorAll('[data-kv]').forEach((el) => (el.oninput = () => { fd[el.dataset.kv][+el.dataset.i][+el.dataset.p] = el.value; }));
  sheet.querySelectorAll('[data-wto]').forEach((el) => (el.onchange = () => { if (el.checked) fd.targets.add(el.dataset.wto); else fd.targets.delete(el.dataset.wto); }));
  document.getElementById('mc-more').ontoggle = (e) => { fd.more = e.target.open; };
});

// leaving JSON mode takes its text into the draft, or says what is wrong with it
function takeJson() {
  const ta = document.getElementById('mc-json');
  if (!ta) return true;
  try { fromJson(ta.value); return true; } catch (e) { toast('The JSON is not a server: ' + e.message, true); return false; }
}
action('mc-cancel', () => { fd = null; go('mcp'); });
action('mc-mode', (el) => { if (el.dataset.v === fd.mode || !takeJson()) return; fd.mode = el.dataset.v; render(); });
action('mc-transport', (el) => { fd.transport = el.dataset.v; render(); });
action('mc-addrow', (el) => { const l = el.dataset.list; fd[l].push(l === 'args' ? '' : ['', '']); render(); });
action('mc-del', (el) => { fd[el.dataset.list].splice(+el.dataset.i, 1); render(); });
action('mc-peek', (el) => { const v = el.previousElementSibling; v.type = v.type === 'password' ? 'text' : 'password'; });
action('mc-save', async () => {
  if (!takeJson()) return;
  const s = server();
  if (!s.name) return toast('Give the server a name', true);
  const rep = await attempt(() => call('mcp_save', { server: s, replace: fd.editing, targets: [...fd.targets] }));
  if (rep === undefined) return;
  syncReport(rep, s.name);
  fd = null;
  go('mcp');
});

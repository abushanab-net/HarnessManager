// System prompts, as the reference draws them: a tab per tool, the saved versions on the left ("The file now" first),
// the editor or the live file on the right. Whether a prompt is in the file is read from the text itself: an enabled
// prompt whose text differs from the file says so, with the write as the next action, and the finding leads the head.

import { S, call, view, toast, attempt, mutate, changed, confirmDialog, action, render, shownTools, patch } from './core.js';
import { esc, toolIcon } from './lib.js';
import { head, badge, field } from './mcp.js';

const FILE = { claude: 'CLAUDE.md', codex: 'AGENTS.md', opencode: 'AGENTS.md', agy: 'GEMINI.md', pi: 'AGENTS.md', omp: 'AGENTS.md' };
let promptTool = 'claude', promptSel = null;
const drafts = {}; // what is typed into the title and body, by prompt id, so a redraw keeps it
const first = (b) => (b || '').split('\n').map((l) => l.replace(/^#+\s*/, '').trim()).find(Boolean) || 'Empty';

view('prompts', async (sheet) => {
  sheet.className = 'sheet';
  const d = await call('items');
  const hs = shownTools(S.state || await call('state')).filter((h) => h.caps.prompts);
  if (!hs.length) return patch(sheet, `<div class="page-body b-page">${head('No tools are shown.', 'Choose which tools to show in Apps.', '<button class="btn-primary" data-act="go-apps">Choose tools</button>')}</div>`);
  if (!hs.some((h) => h.id === promptTool)) promptTool = hs[0].id;
  const t = hs.find((h) => h.id === promptTool), file = FILE[t.id] || 'its prompt file';
  const list = d.prompts.filter((p) => p.harness === promptTool);
  const live = await call('prompt_live', { harness: promptTool }).catch(() => null);
  const sel = list.find((p) => p.id === promptSel) || null, draft = sel ? (drafts[sel.id] || sel) : null;
  const on = list.find((p) => p.enabled), inFile = (p) => !!(live && p.enabled && p.body === live.body);
  // the finding: the enabled prompt that is not what the file holds comes first; else which file the tool reads
  const h1 = on && live && !inFile(on) ? `${esc(on.title)} is enabled, but ${file} holds other text.` : live ? `${file} is the file ${esc(t.label)} reads.` : 'No prompt file yet.';
  const state = (p) => (inFile(p) ? badge('In the file', 'good') : p.enabled ? badge('Enabled · differs from the file', 'warn', 'Saved here with other text than the file holds; write it to apply') : '');
  patch(sheet, `<div class="page-body b-page" data-b-page="prompts">${head(h1, `${list.length} saved ${list.length === 1 ? 'version' : 'versions'} · previous file saved before replacement`, '<button class="btn-primary" data-act="prompt-new" data-testid="prompt-new">New prompt</button>')}
    <nav class="page-tabs" aria-label="Prompt tool" data-testid="prompt-tabs">${hs.map((h) => `<button class="filter-chip ${h.id === promptTool ? 'active' : ''}" data-act="prompt-tool" data-v="${h.id}" title="${esc(h.label)} reads ${FILE[h.id] || 'its prompt file'}" aria-current="${h.id === promptTool ? 'page' : 'false'}">${toolIcon(h.id, 14)} ${esc(h.label)}</button>`).join('')}</nav>
    <div class="b-prompt-grid"><section class="panel-card b-prompt-list" aria-label="Saved prompt versions">
      <button class="b-prompt-choice ${!sel ? 'active' : ''}" data-act="prompt-sel" data-id="" data-key="live"><b>The file now</b>${live ? badge('Live', 'good') : badge('No file')}<span class="muted">${esc(live ? first(live.body) : 'not there yet')}</span></button>
      ${list.map((p) => `<button class="b-prompt-choice ${sel && sel.id === p.id ? 'active' : ''}" data-act="prompt-sel" data-id="${esc(p.id)}" data-key="${esc(p.id)}" data-testid="prompt-${esc(p.id)}"><b>${esc(p.title)}</b>${state(p)}<span class="muted">${esc(first(p.body))}</span></button>`).join('') || '<span class="muted">No saved prompts.</span>'}</section>
    <section class="panel-card b-editor">${sel ? `${field('pr-title', 'Title', draft.title, { extra: 'data-f="title" aria-label="Prompt name"' })}${state(sel)}` : `<div class="panel-head"><h2 class="panel-title">The file now</h2>${live ? badge('Live', 'good') : badge('No file')}</div>`}
      <div class="muted">${live ? `Every ${esc(t.label)} account follows this file` : `No profile of ${esc(t.label)} yet`}</div><div class="mono b-command">${esc(live ? live.path : '—')}</div>
      ${sel ? `<label class="b-sr" for="pr-body">Prompt text</label><textarea id="pr-body" rows="16" data-f="body">${esc(draft.body)}</textarea>
        <div class="b-toolbar"><span class="muted">${inFile(sel) ? 'This prompt is in the file now.' : sel.enabled ? 'Saved here, but the file holds other text.' : 'Not in the file yet.'}</span>
          <button class="btn-secondary" data-act="prompt-delete" data-id="${esc(sel.id)}">Delete…</button><button class="btn-secondary" data-act="prompt-sel" data-id="">Show live file</button>
          ${inFile(sel) ? '' : `<button class="btn-secondary" data-act="prompt-enable" data-id="${esc(sel.id)}" data-testid="prompt-enable">${sel.enabled ? 'Write to the file' : 'Use this prompt'}</button>`}<button class="btn-primary" data-act="prompt-save" data-id="${esc(sel.id)}" data-testid="prompt-save">Save</button></div>`
      : `<pre class="b-live-prompt" tabindex="0" aria-label="The file's text">${esc(live ? live.body || '(the file is empty)' : `No profile of ${t.label} yet.`)}</pre>`}</section></div></div>`);
});
document.addEventListener('input', (e) => {
  const el = e.target, f = el.dataset && el.dataset.f;
  if (!promptSel || !(el.id === 'pr-title' && f === 'title') && !(el.id === 'pr-body' && f === 'body')) return;
  (drafts[promptSel] ||= { title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value })[f] = el.value;
});
const typed = () => ({ title: document.getElementById('pr-title').value, body: document.getElementById('pr-body').value });
action('prompt-tool', (el) => { promptTool = el.dataset.v; promptSel = null; render(); });
action('prompt-sel', (el) => { promptSel = el.dataset.id || null; render(); });
action('prompt-new', async () => { const id = await attempt(() => call('prompt_add', { harness: promptTool, title: 'New prompt', body: '' })); if (id) promptSel = id; await changed(); document.getElementById('pr-title')?.focus(); });
action('prompt-save', async (el) => {
  const v = typed();
  if (!v.title.trim()) return toast('Give the prompt a title.', true);
  if (await mutate(() => call('prompt_save', { id: el.dataset.id, ...v }), 'Saved')) delete drafts[el.dataset.id];
});
// saves, then writes the file; what the file held before is saved as a prompt first (the backend does that)
action('prompt-enable', async (el) => {
  if (!(await attempt(() => call('prompt_save', { id: el.dataset.id, ...typed() })))) return;
  delete drafts[el.dataset.id];
  mutate(() => call('prompt_enable', { id: el.dataset.id }), 'Written to the prompt file');
});
action('prompt-delete', async (el) => { if (await confirmDialog('Delete prompt', 'Delete this prompt from the library? The prompt file is not changed.', 'Delete')) { if (await attempt(() => call('prompt_delete', { id: el.dataset.id }))) { delete drafts[el.dataset.id]; promptSel = null; } changed(); } });

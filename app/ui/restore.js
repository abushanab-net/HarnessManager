// Restore and Welcome back: the history in a backup zip or a sync folder merged into this computer. The preview
// says what it holds; the dialog offers to continue as the computer named in it (adopt), to keep it as a past
// computer, or to leave it. Nothing is counted twice, so restoring again adds nothing.

import { S, call, action, attempt, changed, toast, openDialog, closeDialog, dlgHead, pickFolder, pickFile } from './core.js';
import { esc, fmt, when } from './lib.js';

const kb = (n) => (n >= 1048576 ? (n / 1048576).toFixed(1) + ' MB' : Math.max(1, Math.round(n / 1024)) + ' KB');
const stamp = (t) => new Date(t).toLocaleString([], { dateStyle: 'medium', timeStyle: 'short' });

/** The dialog for a source ({zip} or {folder}); `welcome` is the form shown when a folder is chosen or at first launch. */
export async function restoreDialog(src, { welcome = false, preview = null } = {}) {
  const p = preview || await attempt(() => call('restore_preview', src));
  if (!p) return false;
  const done = () => { if (welcome) call('restore_welcome', { done: true }).catch(() => {}); };
  if (p.empty) { toast(welcome ? 'The folder holds no history yet' : 'Nothing to restore in it'); done(); return false; }
  const name = p.this && p.this.name, comps = p.computers.filter((c) => c.name || c.id);
  // a backup or folder that already knows this computer by its id: restored as what it is, not as a new computer
  const mine = !name && p.computers.some((c) => c.id === ((S.state && S.state.settings) || {}).device_id);
  const intro = p.source === 'folder'
    ? `This folder has the history of ${comps.length} ${comps.length === 1 ? 'computer' : 'computers'}${name ? `, including <b>${esc(name)}</b>` : ''}${p.newest ? `, up to ${esc(stamp(p.newest))}` : ''}.`
    : `This backup is from <b>${esc((comps[0] || {}).name || 'a computer')}</b>${p.newest ? `, up to ${esc(stamp(p.newest))}` : ''}.`;
  openDialog(`${dlgHead(welcome ? 'Welcome back' : 'Restore')}
    <div class="sdlg" data-testid="${welcome ? 'welcome-back' : 'restore-dialog'}">
      <p>${intro} ${welcome ? 'Restore it into this installation?' : 'Merge it into this computer?'}</p>
      <ul class="rlist" data-testid="restore-counts"><li><b>${fmt(p.tokens)}</b> tokens across <b>${p.requests.toLocaleString()}</b> requests, <b>${p.sessions.toLocaleString()}</b> sessions, <b>${p.accounts}</b> ${p.accounts === 1 ? "account's" : "accounts'"} limit history, and <b>${p.archived.toLocaleString()}</b> archived transcripts.</li>
        <li>Keys and logins are not in it: each account asks you to log in again when first used.</li>
        <li>Nothing is counted twice: what this computer already has stays as it is.</li></ul>
      ${comps.length ? `<p class="hint" data-testid="restore-computers">${comps.map((c) => `${esc(c.name || c.id)} · ${c.requests.toLocaleString()} requests · ${c.sessions} sessions`).join('<br>')}</p>` : ''}
      <p class="hint mono muted" style="overflow-wrap:anywhere">${esc(p.path)}</p>
    </div>
    <div class="dfoot"><span class="acts"><button class="btn" data-act="dialog-close" data-testid="welcome-fresh">${welcome ? 'Start fresh' : 'Cancel'}</button>
      <button class="btn ${name ? '' : 'primary'}" id="rs-new" data-testid="welcome-new">${mine ? 'Restore' : 'Restore as a new computer'}</button>
      ${name ? `<button class="btn primary" id="rs-adopt" data-testid="welcome-adopt">Restore and continue as ${esc(name)}</button>` : ''}</span></div>`, { onClose: done });
  const run = async (adopt) => {
    const btns = [...document.querySelectorAll('#rs-new, #rs-adopt')];
    btns.forEach((b) => { b.disabled = true; });
    const r = await attempt(() => call('restore_run', { ...src, adopt }));
    if (!r) { btns.forEach((b) => { b.disabled = false; }); return; }
    closeDialog();
    toast(`Restored: ${r.usage.toLocaleString()} requests, ${r.sessions} sessions and ${r.archived} archived transcripts added${r.adopted ? `; this computer continues as ${r.device.name}` : ''}`);
    changed();
  };
  document.getElementById('rs-new').onclick = () => run(false);
  const ad = document.getElementById('rs-adopt');
  if (ad) ad.onclick = () => run(true);
  return true;
}

/** Welcome back for a sync folder just chosen: the preview, and the dialog when it holds history. */
export const welcomeBack = (folder) => restoreDialog({ folder }, { welcome: true });

// first launch: a sync folder already set and nothing published from here yet
call('restore_welcome').then((p) => { if (p && !p.empty) restoreDialog({ folder: p.path }, { welcome: true, preview: p }); }).catch(() => {});

// Restore…: this computer's backups, the sync folder's, the sync folder itself, or a folder or zip from elsewhere
action('restore-pick', async () => {
  const b = await attempt(() => call('backups'));
  if (!b) return;
  const c = S.cache.computers, folder = c && c.mode === 'folder' ? c.folder : '', sep = b.dir.includes('/') ? '/' : '\\';
  const item = (label, sub, data) => `<a class="rsrc" data-act="restore-src" ${data} role="button" tabindex="0"><b>${label}</b><small>${sub}</small></a>`;
  openDialog(`${dlgHead('Restore from…')}<div class="sdlg rpick" data-testid="restore-pick">
    ${folder ? item('The sync folder', `every computer's journal, the backups and the archive in <span class="mono">${esc(folder)}</span>`, `data-folder="${esc(folder)}" data-testid="restore-src-folder"`) : ''}
    ${b.list.map((x) => item(`${esc(x.label)} · ${esc(stamp(x.at))}`, `a backup of this computer · ${kb(x.size)}`, `data-zip="${esc(b.dir + sep + x.name)}" data-testid="restore-src-${esc(x.label)}"`)).join('')}
    ${(b.in_sync_folder || []).map((x) => item(`${esc(x.label)} · ${esc(stamp(x.at))}`, `a backup of ${esc(x.device_name)} in the sync folder · ${kb(x.size)}`, `data-zip="${esc(x.path)}"`)).join('')}
    ${item('A folder…', 'a sync folder anywhere', 'data-pick="folder" data-testid="restore-src-pick-folder"')}
    ${item('A zip file…', 'a backup exported or copied from elsewhere', 'data-pick="zip" data-testid="restore-src-pick-zip"')}
  </div>`);
});
action('restore-src', async (el) => {
  let src;
  if (el.dataset.pick === 'folder') { const p = await pickFolder(); if (!p) return; src = { folder: p }; }
  else if (el.dataset.pick === 'zip') { const p = await pickFile({ name: 'Backup', extensions: ['zip'] }); if (!p) return; src = { zip: p }; }
  else src = el.dataset.zip ? { zip: el.dataset.zip } : { folder: el.dataset.folder };
  restoreDialog(src);
});


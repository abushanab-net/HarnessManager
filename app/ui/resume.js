// Resume here: a session from another computer, read here and continued here as a fork in this computer's
// clone of its repository. The dialog says what comes across and what does not; the core does the rest.

import { call, toast, attempt, changed, acctName, openDialog, closeDialog, dlgHead, action } from './core.js';
import { esc, when, ic } from './lib.js';

const TOOL = { claude: 'Claude Code', codex: 'Codex', opencode: 'OpenCode', agy: 'Antigravity', pi: 'Pi', omp: 'Oh My Pi' };

/** The Resume-here dialog for a session row from another computer (`s`: the row as `sessions` returned it). */
export async function resumeHereDialog(s) {
  const from = s.device_name || 'the other computer';
  // this computer's clone of the repository: the project's root, as the Projects list knows it
  const projects = await call('projects', {}).catch(() => []);
  const root = ((projects || []).find((p) => p.key === s.project_key) || {}).root || '';
  openDialog(`${dlgHead(s.title || 'Untitled session')}
    <div class="dlg-text a-dlg" data-testid="resume-here-dialog">
      <div class="muted">${esc(s.project_name || s.project_key || 'no project')} · ${esc(acctName(s.profile))} · ${esc(TOOL[s.harness] || s.harness)}</div>
      <dl class="facts-grid"><dt>Last ran on</dt><dd><b>${esc(from)}</b> · ${esc(when(s.last_active_at))}</dd><dt>Opens in</dt><dd class="mono" data-testid="resume-here-root">${esc(root || 'this computer\'s clone of ' + (s.project_name || s.project_key))}</dd></dl>
      <span>Opens as a new session that continues from the ${esc(from)} transcript; the ${esc(from)} copy stays as it is.</span>
      <span class="a-hint">Only the conversation comes across: files changed since, attachments and running shells do not. Pull the branch first.</span>
    </div>
    <div class="modal-foot"><span class="acts"><button class="btn-secondary" data-act="dialog-close">Cancel</button><button class="btn-primary" id="rh-ok" data-testid="resume-here-ok">${ic.play} Resume here</button></span></div>`, { label: 'Resume here' });
  document.getElementById('rh-ok').onclick = async () => {
    const r = await attempt(() => call('session_resume_here', { device: s.device, harness: s.harness, profile: s.profile, id: s.id }));
    if (!r) return;
    closeDialog();
    toast(r.note || 'Resumed in a new terminal');
    changed();
  };
}

// the row's button and the session page's carry the row as JSON
action('resume-here', (el) => resumeHereDialog(JSON.parse(el.dataset.s)));

/** The JSON a Resume-here button carries: what the dialog shows and what the op needs. */
export const resumeData = (s) => esc(JSON.stringify({ device: s.device, device_name: s.device_name, harness: s.harness, profile: s.profile, id: s.id, title: s.title, project_key: s.project_key, project_name: s.project_name, last_active_at: s.last_active_at }));

/** The short form of why a session cannot be resumed here ("not cloned here", "running on LAPTOP"). */
export const reasonShort = (r) => String(r || '').split(':')[0];

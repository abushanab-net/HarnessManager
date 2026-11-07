// Designer C (c-opencode) — Shell, Modals, Tray Popup & Interactive Controller
(function () {
  const D = window.HM_DATA;
  const V = window.HM_VIEWS;

  const navIcon = (id) => `<svg class="nav-icon" viewBox="0 0 24 24" aria-hidden="true">${({overview:'<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><rect x="14" y="14" width="7" height="7" rx="1"/>',sessions:'<path d="m5 6 5 6-5 6m8 0h6"/>',projects:'<path d="M3 7h7l2-3h9v16H3z"/>',accounts:'<circle cx="12" cy="8" r="4"/><path d="M4 21v-3a8 8 0 0 1 16 0v3"/>',setup:'<path d="m4 4 16 16M15 5l4-2 2 2-2 4-5 1-4 4-1 5-4 2-2-2 2-4 5-1 4-4z"/>',cli:'<rect x="3" y="4" width="18" height="16" rx="2"/><path d="m6 8 4 4-4 4m7 0h5"/>',settings:'<path d="M4 7h16M4 17h16"/><circle cx="9" cy="7" r="3"/><circle cx="15" cy="17" r="3"/>'})[id]}</svg>`;

  const STORAGE_THEME_KEY = 'hm-c-opencode-theme';
  const params = new URLSearchParams(window.location.search);

  const STATE = {
    theme: params.get('theme') || (() => { try { return localStorage.getItem(STORAGE_THEME_KEY); } catch { return null; } })() || 'dark',
    frameWidth: Number(params.get('w')) || 1536,
    preset: params.get('preset') || '8', // '8' | '12' | 'firstrun' | 'empty'
    motion: params.get('motion') === 'still' ? 'still' : 'live',
    page: params.get('page') || 'overview', // 'overview' | 'accounts' | 'placeholder'
    placeholderTitle: 'Sessions',
    preferExpiring: false,
    showUsedPct: false,
    windowMode: 'both', // 'both' | '5h' | 'week'
    toolFilter: 'all',
    computerFilter: 'all',
    livePcFilter: 'all',
    readoutRange: 'today',
    yearClockPc: 'all',
    selectedAccountId: 'codex-main',
    searchQuery: '',
    // Left rail folds
    foldPinnedProjects: false,
    foldSetup: true,
    // Accounts page state (daily chart starts compactly folded so account rows sit above fold at 1280)
    acctRange: '7d',
    acctToolFilter: 'all',
    foldDaily: false,
    foldModels: false,
    collapsedTools: {},
    // First-run sync choice
    syncChoice: 'folder',
    // Active modal: null | 'add' | 'detail' | 'folder' | 'reqlog' | 'computers' | 'cli' | 'tour' | 'acctmenu' | 'follow'
    modal: params.get('modal') || null,
    detailAccountId: params.get('account') || 'service-account-resal',
    detailTab: params.get('tab') || 'overview', // 'overview' | 'sessions' | 'projects' | 'settings'
    followSessionId: 's1',
    // Add account form state
    addTool: 'claude',
    addKind: 'subscription',
    addName: 'work-design',
    addEmail: '',
    addApiKey: '',
    addEndpoint: 'https://api.anthropic.com',
    addModel: 'claude-sonnet-4-5',
    addRoute: 'cheap-fast',
    addStep: 'form', // 'form' | 'oauth-code'
    oauthCode: '',
    // Start in folder modal state
    startFolderAcct: 'service-account-resal',
    startFolderPath: '',
    startSkipPerms: true,
    startExtraArgs: '--continue',
    // Tray popup open
    trayOpen: params.get('tray') === '1',
    // Toast / Notice bar
    toastText: null,
    customAccounts: [],
    removedAccountIds: []
  };

  let prevModal = null;
  let lastModalOpenerSelector = null;

  function buildElementSelector(el) {
    if (!el || el === document.body || el === document.documentElement) return null;
    if (el.id) return `#${CSS.escape(el.id)}`;
    const parts = [el.tagName.toLowerCase()];
    ['data-action', 'data-account', 'data-tool', 'data-kind', 'data-page', 'data-w', 'data-theme', 'data-preset', 'data-tab', 'data-range', 'data-pc', 'data-mode', 'data-session', 'data-modal', 'data-expiring', 'data-sync', 'data-field', 'data-save', 'data-msg', 'data-project', 'data-key'].forEach((attr) => {
      if (el.hasAttribute(attr)) {
        parts.push(`[${attr}="${CSS.escape(el.getAttribute(attr))}"]`);
      }
    });
    return parts.length > 1 ? parts.join('') : null;
  }

  function getActiveAccounts() {
    let list;
    if (STATE.preset === 'empty') {
      list = STATE.customAccounts.slice();
    } else if (STATE.preset === 'firstrun') {
      list = D.BASE_ACCOUNTS.filter((a) => a.isMain).map((a) => Object.assign({}, a, {
        measured: false,
        readAgoSec: null,
        windows: [],
        cap: null, route: null, tokens7d: 0, cost7d: 0, modelsBreakdown: [], spark7d: [0,0,0,0,0,0,0],
        openSessions: 0, computers: [], firstReplySec: null, advice: '— Limits not read. No forecast available.', adviceTone: 'neutral'
      })).concat(STATE.customAccounts);
    } else if (STATE.preset === '12') {
      list = D.BASE_ACCOUNTS.concat(D.EXTRA_4_ACCOUNTS, STATE.customAccounts);
    } else {
      list = D.BASE_ACCOUNTS.concat(D.customAccounts || [], STATE.customAccounts);
    }
    return list.filter((a) => !STATE.removedAccountIds.includes(a.id));
  }

  function computeAddCliCmd() {
    const toolObj = D.TOOLS.find((t) => t.id === STATE.addTool) || D.TOOLS[0];
    const safeName = (STATE.addName || 'new-profile').replace(/[^a-z0-9-_]/gi, '').toLowerCase() || 'profile';
    const cliCmd = STATE.addKind === 'subscription'
      ? `hrm add ${safeName} --tool ${toolObj.id}${STATE.addEmail ? ` --email ${D.esc(STATE.addEmail)}` : ''}`
      : STATE.addKind === 'apikey'
        ? `hrm add ${safeName} --tool ${toolObj.id} --kind api-key --endpoint ${D.esc(STATE.addEndpoint)}`
        : `hrm add ${safeName} --tool ${toolObj.id} --route ${STATE.addRoute}`;
    return { toolObj, safeName, cliCmd };
  }

  function renderModal(accounts, metrics) {
    if (!STATE.modal) return '';

    if (STATE.modal === 'add') {
      const { toolObj, safeName, cliCmd } = computeAddCliCmd();
      const kindInfo = toolObj.kinds[STATE.addKind] || toolObj.kinds.subscription;

      return `<div class="modal-backdrop" data-action="close-modal-backdrop">
        <div class="modal-card" role="dialog" aria-modal="true" aria-label="Add account">
          <div class="modal-head">
            <h2>Add account</h2>
            <button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button>
          </div>

          ${STATE.toastText ? `<div class="notice-bar" role="status">${D.esc(STATE.toastText)}</div>` : ''}
          <!-- Step 1: Tool Selector with explicit Kind Compatibility badges -->
          <div class="form-field">
            <label>1. Choose AI Coding Tool</label>
            <div class="tool-pick-grid">
              ${D.TOOLS.map((t) => {
                const sup = [];
                if (t.kinds.subscription.supported) sup.push('Subscription');
                if (t.kinds.apikey.supported) sup.push('API key');
                if (t.kinds.routed.supported) sup.push('Routed');
                return `<button type="button" class="tool-pick-card ${STATE.addTool === t.id ? 'active' : ''}" data-action="set-add-tool" data-tool="${t.id}">
                  <div style="display:flex;align-items:center;justify-content:space-between;width:100%">
                    <span style="display:inline-flex;align-items:center;gap:6px;font-weight:700;font-size:12px">${D.LOGOS[t.id]} ${t.name}</span>
                    <span class="plan-pill" style="font-size:11px">${STATE.preset !== 'empty' && t.installed ? 'v' + t.ver : 'Prepare'}</span>
                  </div>
                  <div style="font-size:11px;color:var(--ink-muted)">Supports: <b style="color:var(--ink-secondary)">${sup.join(' · ')}</b></div>
                </button>`;
              }).join('')}
            </div>
          </div>

          <!-- Step 2: Account Kind (Clearly shows supported vs unsupported per tool) -->
          <div class="form-field">
            <label>2. Account Kind for ${toolObj.name}</label>
            <div class="kind-pick-grid">
              ${[
                ['subscription', 'Subscription', toolObj.kinds.subscription.windowsDesc],
                ['apikey', 'API key', 'Bring your own key with optional endpoint & spend cap'],
                ['routed', 'Routed', 'Route requests via local Harness Router (127.0.0.1:8787)']
              ].map(([k, title, desc]) => {
                const kObj = toolObj.kinds[k];
                const disabled = !kObj.supported;
                return `<button type="button" class="kind-pick-card ${STATE.addKind === k ? 'active' : ''} ${disabled ? 'disabled' : ''}" data-action="set-add-kind" data-kind="${k}" ${disabled ? 'disabled' : ''}>
                  <div style="display:flex;align-items:center;justify-content:space-between;width:100%">
                    <b style="font-size:12px">${title}</b>
                    <span class="plan-pill" style="font-size:11px;color:${disabled ? 'var(--coral)' : 'var(--teal)'}">${disabled ? 'Unsupported' : 'Supported ✓'}</span>
                  </div>
                  <div style="font-size:11px;color:var(--ink-muted)">${disabled ? kObj.reason : desc}</div>
                </button>`;
              }).join('')}
            </div>
          </div>

          ${STATE.addStep === 'oauth-code' ? `
            <div class="panel-card" style="background:var(--teal-dim);border-color:var(--teal-border)">
              <div class="panel-title">Complete ${toolObj.name} Browser Sign-In</div>
              <p style="font-size:12px;color:var(--ink-secondary)">
                Sign-in preview for <b>${safeName}</b>. Paste a code or callback URL below${STATE.addEmail ? ` to verify <b>${D.esc(STATE.addEmail)}</b>` : ''}:
              </p>
              <div class="form-field">
                <label>Verification Code</label>
                <input id="add-oauth-input" type="text" value="${D.esc(STATE.oauthCode)}" placeholder="Paste code from browser…" oninput="window.HM_APP.updateField('oauthCode', this.value)"/>
              </div>
            </div>
          ` : `
            <!-- Step 3: Profile Inputs -->
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">
              <div class="form-field">
                <label>Profile Name (creates folder <span class="mono" data-bind="add-folder-preview" style="color:var(--teal)">${toolObj.folderPrefix}${safeName}</span>)</label>
                <input id="add-name-input" type="text" value="${D.esc(STATE.addName)}" placeholder="work-design" oninput="window.HM_APP.updateField('addName', this.value)"/>
              </div>
              ${STATE.addKind === 'subscription' ? `
                <div class="form-field">
                  <label>Expected Email <span style="font-weight:400;color:var(--ink-muted)">(optional — verified after sign-in)</span></label>
                  <input id="add-email-input" type="email" value="${D.esc(STATE.addEmail)}" placeholder="you@company.com (optional)" oninput="window.HM_APP.updateField('addEmail', this.value)"/>
                </div>
              ` : STATE.addKind === 'apikey' ? `
                <div class="form-field">
                  <label>API Key (${kindInfo.envVar || 'API_KEY'})</label>
                  <input id="add-apikey-input" type="password" value="${D.esc(STATE.addApiKey)}" placeholder="sk-..." oninput="window.HM_APP.updateField('addApiKey', this.value)"/>
                </div>
              ` : `
                <div class="form-field">
                  <label>Local Router Route (:8787)</label>
                  <select id="add-route-select" onchange="window.HM_APP.updateField('addRoute', this.value)">
                    ${D.ROUTER_ROUTES.map((rt) => `<option value="${rt.id}" ${STATE.addRoute === rt.id ? 'selected' : ''}>${rt.name} — ${rt.target} (${rt.costNote})</option>`).join('')}
                  </select>
                </div>
              `}
            </div>

            ${STATE.addKind !== 'subscription' ? `
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">
                <div class="form-field">
                  <label>Optional Endpoint</label>
                  <input id="add-endpoint-input" type="text" value="${D.esc(STATE.addEndpoint)}" oninput="window.HM_APP.updateField('addEndpoint', this.value)"/>
                </div>
                <div class="form-field">
                  <label>Optional Default Model</label>
                  <input id="add-model-input" type="text" value="${D.esc(STATE.addModel)}" oninput="window.HM_APP.updateField('addModel', this.value)"/>
                </div>
              </div>
            ` : ''}
          `}

          <!-- Sync vs Isolated Facts + CLI Equivalent -->
          <div class="sync-facts-grid">
            <div>
              <b style="color:var(--teal);display:block;margin-bottom:2px">Kept in sync from main profile (<span class="mono">${toolObj.mainFolder}</span>)</b>
              <span style="color:var(--ink-secondary)">${toolObj.syncedItems}</span>
            </div>
            <div>
              <b style="color:var(--ink-primary);display:block;margin-bottom:2px">Isolated to this account only</b>
              <span style="color:var(--ink-secondary)">${toolObj.isolatedItems}</span>
            </div>
          </div>

          <div style="display:flex;align-items:center;justify-content:space-between;padding:7px 10px;border-radius:6px;background:var(--bg-input);border:1px solid var(--line-subtle)">
            <span style="font-size:11px;color:var(--ink-muted)">CLI equivalent</span>
            <code class="mono" data-bind="add-cli-preview" style="font-size:11.5px;color:var(--teal)">${D.esc(cliCmd)}</code>
          </div>

          <div style="display:flex;align-items:center;justify-content:space-between;padding-top:6px;border-top:1px solid var(--line-subtle)">
            <span></span>
            <div style="display:flex;gap:8px">
              <button type="button" class="btn-secondary" data-action="close-modal">Cancel</button>
              <button type="button" class="btn-primary" data-action="submit-add-account">
                ${STATE.addStep === 'oauth-code'
                  ? 'Verify code & finish'
                  : STATE.addKind === 'subscription'
                    ? (toolObj.id === 'claude' ? 'Create & sign in (browser code)…' : 'Create & sign in (terminal)…')
                    : 'Add account'}
              </button>
            </div>
          </div>
        </div>
      </div>`;
    }

    if (STATE.modal === 'follow') {
      const s = D.LIVE_SESSIONS.find((x) => x.id === STATE.followSessionId) || D.LIVE_SESSIONS[0];
      const pc = D.COMPUTERS.find((c) => c.id === s.computer) || D.COMPUTERS[0];
      const acct = accounts.find((a) => a.id === s.accountId) || D.BASE_ACCOUNTS.find((a) => a.id === s.accountId) || D.BASE_ACCOUNTS[0];
      const w5 = (acct.windows || []).find((w) => w.kind === '5h');
      const wk = (acct.windows || []).find((w) => w.kind === 'week');

      return `<div class="modal-backdrop" data-action="close-modal-backdrop">
        <div class="modal-card" style="max-width:680px" role="dialog" aria-modal="true" aria-label="Follow live session">
          <div class="modal-head">
            <div style="display:flex;align-items:center;gap:9px">
              <i class="live-pulse-dot"></i>
              <div>
                <h2 style="display:flex;align-items:center;gap:7px">
                  ${D.LOGOS[s.tool]}
                  <span>${s.project}</span>
                  <span class="plan-pill">${s.branch}</span>
                </h2>
                <div style="font-size:11.5px;color:var(--ink-muted)">
                  Running on <b>${pc.name}</b> · Account <b>${s.accountId}</b> · Model <b>${s.model}</b> · ${s.sinceMin}m elapsed
                </div>
              </div>
            </div>
            <button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button>
          </div>

          <div class="sync-facts-grid" style="grid-template-columns:1fr 1fr">
            <div>
              <b style="color:var(--teal);display:block;margin-bottom:2px">Account Runway (${D.esc(acct.name)})</b>
              <span style="color:var(--ink-secondary)">
                ${w5 ? `5h: ${w5.left}% left (↻ ${D.fmtDuration(w5.resetMs)}) · ` : ''}${wk ? `Week: ${wk.left}% left (↻ ${D.fmtDuration(wk.resetMs)})` : '— limits not read'}
              </span>
            </div>
            <div>
              <b style="color:var(--ink-primary);display:block;margin-bottom:2px">Session Telemetry</b>
              <span style="color:var(--ink-secondary)">${D.fmtTokens(s.tokensSession)} tokens in session · Status: <b style="color:var(--teal)">${s.status}</b></span>
            </div>
          </div>

          <div class="panel-card" style="background:var(--bg-input);border-color:var(--line-default);font-size:11.5px;gap:6px">
            <div style="display:flex;justify-content:space-between;color:var(--ink-muted)">
              <span>Live session output tail (${pc.name})</span>
              <span class="mono">pid #48210</span>
            </div>
            <div class="mono" style="display:flex;flex-direction:column;gap:4px;color:var(--ink-secondary);padding-top:4px;border-top:1px solid var(--line-subtle)">
              <div><span style="color:var(--ink-muted)">[15:40:12]</span> ✓ Read workspace configuration &amp; active branch (${s.branch})</div>
              <div><span style="color:var(--ink-muted)">[15:41:04]</span> ✓ Applied patch across 3 files (+84 −19 lines)</div>
              <div><span style="color:var(--teal)">[15:41:52]</span> ▸ ${s.lastAction}…</div>
            </div>
          </div>

          <div style="display:flex;justify-content:space-between;align-items:center;padding-top:4px">
            <button class="btn-secondary" data-action="open-account-detail" data-account="${acct.id}">Inspect ${D.esc(acct.name)}</button>
            <div style="display:flex;gap:8px">
              <button class="btn-secondary" data-action="close-modal">Close</button>
              <button class="btn-primary" data-action="toast-and-close" data-msg="Focused terminal window for ${s.project} on ${pc.name}.">Focus Terminal Window</button>
            </div>
          </div>
        </div>
      </div>`;
    }

    if (STATE.modal === 'signin') {
      const acct = accounts.find((a) => a.id === STATE.detailAccountId);
      if (!acct) return '';
      const tool = D.TOOLS.find((t) => t.id === acct.tool);
      return `<div class="modal-backdrop" data-action="close-modal-backdrop"><div class="modal-card" role="dialog" aria-modal="true" aria-label="Sign in again"><div class="modal-head"><h2>Sign in to ${tool.name}</h2><button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button></div><div class="notice-bar" role="status">${STATE.signInSent ? 'Code received in preview. Login awaits live verification.' : 'Sign-in preview · no vendor connection'}</div><span>${D.esc(acct.name)}${acct.expectedEmail ? ' · expected ' + D.esc(acct.expectedEmail) : ''}</span>${tool.id === 'claude' ? `<button class="btn-secondary" data-action="preview-signin-page">Open the sign-in page (preview)</button><span class="mono">https://claude.ai/oauth/authorize</span><div class="form-field"><label for="signin-code">Code or callback URL</label><input id="signin-code" value="${D.esc(STATE.signInCode || '')}" placeholder="Paste the code or callback URL" oninput="window.HM_APP.updateField('signInCode',this.value)"/></div><button class="btn-secondary" data-action="send-signin-code">Send</button>` : '<span>Terminal sign-in preview. No terminal was opened; finish sign-in there when connected to the app.</span>'}<div class="page-actions"><button class="btn-secondary" data-action="close-modal">Cancel sign-in</button><button class="btn-primary" data-action="close-modal">Done</button></div></div></div>`;
    }

    if (STATE.modal === 'remove') {
      const acct = accounts.find((a) => a.id === STATE.detailAccountId);
      if (!acct) return '';
      return `<div class="modal-backdrop" data-action="close-modal-backdrop"><div class="modal-card" role="dialog" aria-modal="true" aria-label="Remove account"><div class="modal-head"><h2>${acct.isMain ? 'Hide main profile' : 'Remove account'}</h2><button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button></div><span>${acct.isMain ? 'Its folder and login are never deleted. Find tools in Settings adds it back.' : 'Removes this profile from the preview. Its folder stays on disk.'}</span>${acct.isMain ? '' : '<label><input type="checkbox" id="remove-folder"/> Also delete its folder (preview only)</label>'}<button class="btn-secondary" data-action="remove-account" data-account="${acct.id}">${acct.isMain ? 'Hide profile' : 'Remove account'}</button></div></div>`;
    }

    if (STATE.modal === 'folder') {
      const acct = accounts.find((a) => a.id === STATE.startFolderAcct) || accounts[0] || D.BASE_ACCOUNTS[0];
      return `<div class="modal-backdrop" data-action="close-modal-backdrop">
        <div class="modal-card" style="max-width:520px" role="dialog" aria-modal="true" aria-label="Start in a folder">
          <div class="modal-head">
            <h2>Start session in a folder…</h2>
            <button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button>
          </div>
          <div class="form-field">
            <label>Account (sorted by room &amp; next reset)</label>
            <select id="start-folder-acct-select" onchange="window.HM_APP.updateField('startFolderAcct', this.value)">
              ${accounts.map((a) => `<option value="${a.id}" ${a.id === acct.id ? 'selected' : ''}>${D.esc(a.name)} (${D.esc(a.plan)}) — ${D.esc(a.advice)}</option>`).join('')}
            </select>
          </div>
          <div class="form-field">
            <label>Recent Folder</label>
            <select id="start-folder-path" class="mono" onchange="window.HM_APP.updateField('startFolderPath', this.value)">
              ${(acct.recentFolders || ['D:\\Projects\\abushanab-net\\HarnessManager']).map((f) => `<option ${STATE.startFolderPath === f ? 'selected' : ''}>${D.esc(f)}</option>`).join('')}
            </select>
          </div>
          <label style="display:flex;align-items:center;gap:8px;font-size:12px;cursor:pointer">
            <input type="checkbox" ${STATE.startSkipPerms ? 'checked' : ''} onchange="window.HM_APP.updateField('startSkipPerms', this.checked)"/>
            <span>Skip permission prompts (<code class="mono">--dangerously-skip-permissions</code> / <code class="mono">--full-auto</code>)</span>
          </label>
          <div class="form-field">
            <label>Extra CLI Arguments</label>
            <input id="start-extra-args-input" class="mono" type="text" value="${D.esc(STATE.startExtraArgs)}" oninput="window.HM_APP.updateField('startExtraArgs', this.value)"/>
          </div>
          <div style="display:flex;justify-content:flex-end;gap:8px">
            <button class="btn-secondary" data-action="close-modal">Cancel</button>
            <button class="btn-primary" data-action="start-account" data-account="${acct.id}">▶ Launch Terminal</button>
          </div>
        </div>
      </div>`;
    }

    if (STATE.modal === 'reqlog') {
      return `<div class="modal-backdrop" data-action="close-modal-backdrop">
        <div class="modal-card" style="max-width:880px" role="dialog" aria-modal="true" aria-label="Full Request Log">
          <div class="modal-head">
            <div>
              <h2>Full Request Log</h2>
              <div style="font-size:11.5px;color:var(--ink-muted)">${metrics.repliesTodayCount.toLocaleString('en-US')} requests today across ${metrics.computersList.length} computer${metrics.computersList.length === 1 ? '' : 's'} · every numeric column right-aligned</div>
            </div>
            <button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button>
          </div>
          <table class="replies-table">
            <thead>
              <tr>
                <th>Time</th>
                <th>Account</th>
                <th>Model</th>
                <th>Computer</th>
                <th class="num">Input</th>
                <th class="num">Output</th>
                <th class="num">Cache Read</th>
                <th class="num">Speed ≈</th>
                <th class="num">First Reply</th>
                <th class="num">Cost ≈</th>
              </tr>
            </thead>
            <tbody>
              ${(metrics.isFresh ? [] : D.LATEST_REPLIES).map((r) => `<tr>
                <td class="mono">${r.time}</td>
                <td>${D.LOGOS[r.tool]} <b>${r.account}</b></td>
                <td class="mono">${r.model}</td>
                <td>${r.computer}</td>
                <td class="num mono">${r.inTok}</td>
                <td class="num mono">${r.outTok}</td>
                <td class="num mono">${r.cacheTok}</td>
                <td class="num mono">${r.tpsModalOnly}</td>
                <td class="num mono">${r.firstReply}</td>
                <td class="num mono">${r.cost}</td>
              </tr>`).join('')}
            </tbody>
          </table>
        </div>
      </div>`;
    }

    if (STATE.modal === 'computers' || STATE.modal === 'cli' || STATE.modal === 'tour' || STATE.modal === 'acctmenu') {
      const titles = {
        computers: `Settings › Computers & Sync (${metrics.computersList.length} Alive)`,
        cli: 'Harness Manager (`hrm`) CLI Reference',
        tour: 'Quick Walkthrough — Chrono-Runway Overview',
        acctmenu: `Account Actions — ${STATE.detailAccountId}`
      };
      return `<div class="modal-backdrop" data-action="close-modal-backdrop">
        <div class="modal-card" style="max-width:560px" role="dialog" aria-modal="true" aria-label="${titles[STATE.modal]}">
          <div class="modal-head">
            <h2>${titles[STATE.modal]}</h2>
            <button class="icon-btn" data-action="close-modal" aria-label="Close">✕</button>
          </div>
          ${STATE.modal === 'computers' ? `
            <div style="display:flex;flex-direction:column;gap:8px">
              ${metrics.computersList.map((c) => {
                const liveCnt = metrics.activeSessions.filter((s) => s.computer === c.id).length;
                return `<div class="live-mini-row" style="padding:9px 12px">
                  <div class="live-mini-left">
                    <i class="pc-dot" style="background:${c.col}"></i>
                    <b>${c.name}</b> ${c.me ? '<span class="plan-pill">This computer</span>' : ''}
                    <span style="color:var(--ink-muted)">${c.os}</span>
                  </div>
                  <span style="color:var(--teal);font-weight:600">${liveCnt} live · seen ${c.seenMsAgo === 0 ? 'just now' : Math.round(c.seenMsAgo / 1e3) + 's ago'}</span>
                </div>`;
              }).join('')}
            </div>
          ` : STATE.modal === 'cli' ? `
            <div style="display:flex;flex-direction:column;gap:7px;font-size:12px">
              <div class="live-mini-row"><code class="mono" style="color:var(--teal)">hrm start</code><span>Starts session on account with most room / expiring 5h window</span></div>
              <div class="live-mini-row"><code class="mono" style="color:var(--teal)">hrm status --windows</code><span>Prints 5-hour &amp; weekly reset runway in terminal</span></div>
              <div class="live-mini-row"><code class="mono" style="color:var(--teal)">hrm add &lt;name&gt; --tool claude</code><span>Creates isolated profile folder and syncs skills/MCP</span></div>
              <div class="live-mini-row"><code class="mono" style="color:var(--teal)">hrm sync now</code><span>Forces immediate limit &amp; session sync across computers</span></div>
            </div>
          ` : STATE.modal === 'acctmenu' ? `
            <div style="display:flex;flex-direction:column;gap:6px">
              ${[
                ['Open profile folder in Explorer', 'toast-and-close', `Opened folder for ${STATE.detailAccountId} in Explorer`],
                ['Rename account label…', 'open-account-detail', STATE.detailAccountId],
                ['Profile settings & cost cap…', 'open-account-detail', STATE.detailAccountId],
                ['Sync skills & MCP servers from main profile now', 'toast-and-close', 'Synced CLAUDE.md, 255 skills & 11 MCP servers'],
                ['Show active terminal window', 'toast-and-close', 'Focused active terminal window'],
                [accounts.find((a) => a.id === STATE.detailAccountId)?.isMain ? 'Hide main profile (keep folder and login)' : 'Remove account…', 'confirm-remove', STATE.detailAccountId]
              ].map(([lbl, actName, param], idx) => `<button class="live-mini-row" style="padding:8px 10px;text-align:left;color:${idx >= 5 ? 'var(--coral)' : 'var(--ink-primary)'}" data-action="${actName}" ${actName === 'toast-and-close' ? `data-msg="${param}"` : `data-account="${param}" data-tab="settings"`}>${lbl}</button>`).join('')}
            </div>
          ` : `
            <div style="display:flex;flex-direction:column;gap:8px;font-size:12.5px;color:var(--ink-secondary)">
              <div><b>1. Top Dispatch Deck:</b> Answers which account to use right now (because it has the most room or its 5h window resets soon), which account is on pace to run out before its reset (<b>codex-main</b>), and what is running across your 3 computers.</div>
              <div><b>2. The Reset Horizon Timeline:</b> Separates pure capacity (% left) from a linear 7-day reset timeline toward each account's exact reset gate (<b>↻</b>), with dashed forecast lines (<b>≈</b>) and hatched starvation gaps.</div>
              <div><b>3. De-crowded Rail &amp; Tray Popup:</b> Limit rings now live directly on each timeline lane and in the scrollable System Tray popup, while dot-grid readouts sit cleanly below the timeline.</div>
            </div>
          `}
        </div>
      </div>`;
    }
    return '';
  }

  function renderTrayPopup(accounts, metrics) {
    if (!STATE.trayOpen) return '';
    const best = metrics.bestRoomAcct;
    const bestW5 = best && best.windows ? best.windows.find((w) => w.kind === '5h') : null;
    return `<aside class="tray-popup" role="dialog" aria-label="System Tray Always-on-Top Popup">
      <div style="display:flex;align-items:center;justify-content:space-between">
        <span style="display:inline-flex;align-items:center;gap:6px;font-weight:700;font-size:12px">
          <span style="width:16px;height:18px;display:inline-block">${D.LOGOS.harness}</span>
          System Tray Popup · ${accounts.length} Accounts
        </span>
        <button class="icon-btn" style="width:22px;height:22px" data-action="toggle-tray" aria-label="Close tray popup">✕</button>
      </div>
      <div style="font-size:11px;color:var(--ink-muted)">
        ${best ? `Use next: <b style="color:var(--teal)">${D.esc(best.name)}</b> ${bestW5 ? `(${bestW5.left}% 5h · ↻ ${D.fmtDuration(bestW5.resetMs)})` : ''}` : accounts.length ? '— Limits not read; no recommendation' : 'No accounts configured'}
      </div>
      <div class="tray-rings-grid">
        ${accounts.map((a) => {
          const wk = (a.windows || []).find((w) => w.kind === 'week');
          return `<div class="tray-ring-card" tabindex="0" data-action="open-account-detail" data-account="${a.id}" style="cursor:pointer" title="Inspect ${D.esc(a.name)}">
            ${V.renderRingSvg(a, STATE.showUsedPct)}
            <div style="min-width:0;flex:1">
              <div style="display:flex;align-items:center;gap:4px">
                ${D.LOGOS[a.tool]}
                <b style="font-size:11px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">${D.esc(a.name)}</b>
              </div>
              <div style="font-size:11px;color:var(--ink-muted)">${wk ? `↻ ${D.fmtDuration(wk.resetMs)}` : a.cap ? 'Daily cap · $' + (a.cap.limitUsd-a.cap.spentUsd).toFixed(2) + ' left' : !['claude','codex'].includes(a.tool) ? 'No plan limits' : a.kind === 'apikey' ? 'Pay per use' : '— unread'}</div>
            </div>
          </div>`;
        }).join('')}
      </div>
    </aside>`;
  }

  function triggerCsvDownload(accounts) {
    const headers = ['account', 'tool', 'kind', 'plan', 'email', '5h_left_pct', 'week_left_pct', 'tokens_7d', 'cost_7d_usd'];
    const rows = accounts.map((a) => {
      const w5 = (a.windows || []).find((w) => w.kind === '5h');
      const wk = (a.windows || []).find((w) => w.kind === 'week');
      return [
        a.name,
        a.tool,
        a.kind,
        a.plan || '',
        a.email || '',
        w5 ? w5.left : '',
        wk ? wk.left : '',
        a.tokens7d || 0,
        (a.cost7d || 0).toFixed(2)
      ].map((value) => `"${String(value).replace(/"/g, '""')}"`).join(',');
    });
    const csvContent = [headers.join(',')].concat(rows).join('\n');
    try {
      const blob = new Blob([csvContent], { type: 'text/csv;charset=utf-8;' });
      const url = URL.createObjectURL(blob);
      const link = document.createElement('a');
      link.href = url;
      link.setAttribute('download', `harness-accounts-${STATE.acctRange}.csv`);
      document.body.appendChild(link);
      link.click();
      document.body.removeChild(link);
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (_) {}
  }

  function nodeKey(node) {
    if (node.nodeType !== 1) return null;
    return node.id || (node.hasAttribute('data-region') ? 'region:' + node.dataset.region : null) || buildElementSelector(node);
  }

  function patchNode(current, next) {
    if (current.nodeType !== next.nodeType || current.nodeName !== next.nodeName) {
      const replacement = next.cloneNode(true);
      current.replaceWith(replacement); return replacement;
    }
    if (current.nodeType !== 1) {
      if (current.nodeValue !== next.nodeValue) current.nodeValue = next.nodeValue;
      return;
    }
    for (const attr of [...current.attributes]) if (!next.hasAttribute(attr.name)) current.removeAttribute(attr.name);
    for (const attr of [...next.attributes]) if (current.getAttribute(attr.name) !== attr.value) current.setAttribute(attr.name, attr.value);
    const old = [...current.childNodes];
    const keyed = new Map(old.filter(nodeKey).map((node) => [nodeKey(node), node]));
    let cursor = current.firstChild;
    for (const child of [...next.childNodes]) {
      const key = nodeKey(child);
      let match = key ? keyed.get(key) : cursor && !nodeKey(cursor) && cursor.nodeName === child.nodeName ? cursor : null;
      if (!match) { match = child.cloneNode(true); current.insertBefore(match, cursor); }
      else { if (match !== cursor) current.insertBefore(match, cursor); match = patchNode(match, child) || match; }
      cursor = match.nextSibling;
    }
    while (cursor) { const nextSibling = cursor.nextSibling; cursor.remove(); cursor = nextSibling; }
    if (current instanceof HTMLInputElement) {
      if (document.activeElement !== current && current.value !== next.value) current.value = next.value;
      current.checked = next.checked;
    }
    if (current instanceof HTMLSelectElement && document.activeElement !== current) current.value = next.value;
  }

  let mountedPage = null;
  let disposePage = null;
  function renderApp() {
    document.documentElement.setAttribute('data-theme', STATE.theme);
    document.documentElement.setAttribute('data-motion', STATE.motion);
    document.documentElement.setAttribute('data-frame-w', String(STATE.frameWidth));
    document.documentElement.style.setProperty('--frame-w', STATE.frameWidth + 'px');
    document.documentElement.style.setProperty('--frame-h', ({1280:720,1536:864,1920:1080}[STATE.frameWidth] || 864) + 'px');
    const root = document.getElementById('app-root');
    if (!root) return;

    // Snapshot scroll & focus before DOM update
    const winScrollY = window.scrollY || document.documentElement.scrollTop || 0;
    const modalElBefore = root.querySelector('.modal-card');
    const modalScrollTop = modalElBefore ? modalElBefore.scrollTop : 0;
    const trayElBefore = root.querySelector('.tray-rings-grid');
    const trayScrollTop = trayElBefore ? trayElBefore.scrollTop : 0;

    const activeEl = document.activeElement;
    const activeSelector = root.contains(activeEl) ? buildElementSelector(activeEl) : null;
    const selStart = activeEl && typeof activeEl.selectionStart === 'number' ? activeEl.selectionStart : null;
    const selEnd = activeEl && typeof activeEl.selectionEnd === 'number' ? activeEl.selectionEnd : null;

    const accounts = getActiveAccounts();
    const metrics = D.deriveMetrics(accounts, STATE.preset);
    STATE.accounts = accounts; STATE.metrics = metrics;
    const scrollPositions = [...root.querySelectorAll('*')].filter((el) => el.scrollTop || el.scrollLeft).map((el) => [el, el.scrollTop, el.scrollLeft]);

    const registeredPage = window.HM.pages[STATE.page];
    const mainContent = STATE.page === 'overview'
      ? ((STATE.preset === 'firstrun' || STATE.preset === 'empty' && !accounts.length) ? V.renderFirstRunOrEmpty(STATE, accounts) : V.renderOverview(STATE, accounts, metrics))
      : STATE.page === 'accounts' ? V.renderAccountsPage(STATE, accounts, metrics)
      : registeredPage ? registeredPage.render(STATE)
      : `<div class="page-body"><section class="panel-card empty-state"><h1>${D.esc(window.HM.titles[STATE.page] || STATE.page)}</h1><span>Page awaiting its round 2 design.</span><button class="btn-secondary" data-action="nav-page" data-page="overview">Back to Overview</button></section></div>`;

    const template = document.createElement('template');
    template.innerHTML = `
      <!-- Top Prototype Studio Framing Bar -->
      <header data-region="studio" class="proto-bar" aria-label="Prototype controls">
        <div class="proto-bar-left">
          <span class="proto-badge">◆ Designer C · Chrono-Runway</span>
          <div class="proto-group" role="group" aria-label="Screen width switch">
            <span class="proto-group-label">Width</span>
            ${[1280, 1536, 1920].map((w) => `<button class="proto-btn ${STATE.frameWidth === w ? 'active' : ''}" data-action="set-width" data-w="${w}">${w}</button>`).join('')}
          </div>
          <div class="proto-group" role="group" aria-label="Theme switch">
            <span class="proto-group-label">Theme</span>
            <button class="proto-btn ${STATE.theme === 'dark' ? 'active' : ''}" data-action="set-theme" data-theme="dark">Harness Dark</button>
            <button class="proto-btn ${STATE.theme === 'light' ? 'active' : ''}" data-action="set-theme" data-theme="light">Harness Light</button>
          </div>
          <div class="proto-group" role="group" aria-label="Dataset preset">
            <span class="proto-group-label">State</span>
            <button class="proto-btn ${STATE.preset === '8' ? 'active' : ''}" data-action="set-preset" data-preset="8">8 Accounts</button>
            <button class="proto-btn ${STATE.preset === '12' ? 'active' : ''}" data-action="set-preset" data-preset="12">12 Accounts</button>
            <button class="proto-btn ${STATE.preset === 'firstrun' ? 'active' : ''}" data-action="set-preset" data-preset="firstrun">First Run</button>
            <button class="proto-btn ${STATE.preset === 'empty' ? 'active' : ''}" data-action="set-preset" data-preset="empty">Empty (0)</button>
          </div>
        </div>
        <div class="proto-bar-right">
          <div class="proto-group" role="group" aria-label="Motion toggle">
            <span class="proto-group-label">Motion</span>
            <button class="proto-btn ${STATE.motion === 'live' ? 'active' : ''}" data-action="set-motion" data-motion="live">Live</button>
            <button class="proto-btn ${STATE.motion === 'still' ? 'active' : ''}" data-action="set-motion" data-motion="still">Still</button>
          </div>
          <button class="proto-btn" style="border:1px solid rgba(255,255,255,0.2)" data-action="simulate-redraw" title="Verify Rule 5: re-render live data without losing selection, scroll, focus, or animation phase">↻ Simulate 3s Redraw</button>
        </div>
      </header>

      <!-- Framed Desktop Window -->
      <div class="stage-wrap">
        <div class="app-frame">
          <!-- Left Navigation Rail -->
          <aside data-region="rail" class="rail" aria-label="Main navigation">
            <div class="rail-top">
              <div class="brand-box">
                <div class="brand-logo">${D.LOGOS.harness}</div>
                <div class="brand-text">
                  <b>Harness Manager</b>
                  <small>DEVELOPMENTPC · ${accounts.length} account${accounts.length === 1 ? '' : 's'}</small>
                </div>
              </div>

              <nav class="nav-section" aria-label="Primary pages">
                <button class="nav-item ${STATE.page === 'overview' ? 'active' : ''}" data-action="nav-page" data-page="overview">
                  <span class="nav-item-left">${navIcon('overview')}<span>Overview</span></span>
                  <span class="nav-badge">${metrics.costTodayVal}</span>
                </button>
                <button class="nav-item ${STATE.page === 'sessions' ? 'active' : ''}" data-action="nav-page" data-page="sessions">
                  <span class="nav-item-left">${navIcon('sessions')}<span>Sessions</span></span>
                  <span class="nav-badge">${metrics.activeSessions.length > 0 ? '<span class="mini-dot-grid"><i></i><i></i><i></i><i></i><i></i><i></i><i></i><i></i></span>' : ''}<span>${metrics.activeSessions.length}</span></span>
                </button>
                <div>
                  <button class="nav-item ${STATE.page === 'projects' ? 'active' : ''}" data-action="nav-page" data-page="projects">
                    <span class="nav-item-left">${navIcon('projects')}<span>Projects</span></span>
                    <span class="nav-badge">${metrics.projectsCount}</span>
                  </button>
                  <button class="filter-chip" data-action="toggle-pinned-projects" aria-label="Toggle pinned projects">${STATE.foldPinnedProjects ? '▾' : '▸'} Pinned</button>
                  ${STATE.foldPinnedProjects && metrics.projectsCount > 0 ? `<div class="nav-sublist">
                    ${D.PINNED_PROJECTS.map((p) => `<button class="nav-subitem" data-action="nav-page" data-page="projects" data-project="${D.esc(p.name)}"><span>${p.name}</span><span style="color:var(--teal)">●${p.live}</span></button>`).join('')}
                  </div>` : ''}
                </div>
                <button class="nav-item ${['accounts','account'].includes(STATE.page) ? 'active' : ''}" data-action="nav-page" data-page="accounts">
                  <span class="nav-item-left">${navIcon('accounts')}<span>Accounts</span></span>
                  <span class="nav-badge">${accounts.length}</span>
                </button>

                <div class="rail-divider"></div>

                <!-- Foldable Setup Group -->
                <div>
                  <button class="nav-item" data-action="toggle-setup-fold">
                    <span class="nav-item-left">${navIcon('setup')}<span>Setup</span></span>
                    <span class="nav-badge warn">${metrics.updatesCount} update${metrics.updatesCount === 1 ? '' : 's'}</span>
                  </button>
                  ${STATE.foldSetup ? `<div class="nav-sublist">
                    <button class="nav-subitem" data-action="nav-page" data-page="mcp"><span>MCP servers</span><span>${metrics.mcpCount}</span></button>
                    <button class="nav-subitem" data-action="nav-page" data-page="skills"><span>Skills</span><span>${metrics.skillsCount}</span></button>
                    <button class="nav-subitem" data-action="nav-page" data-page="prompts"><span>System prompts</span><span></span></button>
                    <button class="nav-subitem" data-action="nav-page" data-page="routing"><span>API keys &amp; routing</span><span></span></button>
                    <button class="nav-subitem" data-action="nav-page" data-page="apps"><span>Apps</span><span style="color:var(--amber);font-weight:600">${metrics.updatesCount} update${metrics.updatesCount === 1 ? '' : 's'}</span></button>
                  </div>` : ''}
                </div>
              </nav>
            </div>

            <div class="rail-bottom">
              <button class="tray-trigger" data-action="toggle-tray" title="Preview the system tray always-on-top limit rings popup">
                <span>◷ Tray limits popup</span>
                <span style="color:var(--teal)">${STATE.trayOpen ? '▾' : '▴'}</span>
              </button>
              <button class="nav-item ${STATE.page === 'cli' ? 'active' : ''}" data-action="nav-page" data-page="cli">
                <span class="nav-item-left">${navIcon('cli')}<span>CLI reference</span></span>
              </button>
              <button class="nav-item ${STATE.page === 'settings' ? 'active' : ''}" data-action="nav-page" data-page="settings">
                <span class="nav-item-left">${navIcon('settings')}<span>Settings</span></span>
                <span class="update-dot" title="Harness Manager v1.8.4 available"></span>
              </button>
              <div class="ver-row">
                <span>v1.8.3</span>
                <span>tray active</span>
              </div>
            </div>
          </aside>

          <!-- Right Workspace -->
          <div class="workspace">
            <header data-region="topbar" class="topbar">
              <label class="search-box">
                <span style="color:var(--ink-muted)">⌕</span>
                <input id="global-search" type="search" value="${D.esc(STATE.searchQuery)}" placeholder="Find an account, session or setting" aria-label="Find an account, session or setting" oninput="window.HM_APP.onSearch(this.value)"/>
                <kbd class="kbd">Ctrl K</kbd>
              </label>

              <div class="topbar-right">
                <button class="status-chip" data-action="nav-page" data-page="sessions">
                  <i class="live-dot"></i>
                  <span style="font-weight:600;color:var(--ink-primary)">${metrics.liveHere} live · ${metrics.liveElsewhere} elsewhere</span>
                </button>
                <button class="status-chip" data-action="nav-page" data-page="settings" data-tab="computers" title="Settings › Computers">
                  <span class="pc-dots">${metrics.computersList.map((c) => `<i class="pc-dot" style="background:${c.col}"></i>`).join('')}</span>
                  <span>${metrics.computersList.length} computer${metrics.computersList.length === 1 ? '' : 's'}</span>
                </button>
                <span class="freshness-label">${metrics.freshnessText}</span>
                <button class="icon-btn" data-action="open-modal" data-modal="tour" title="Page walkthrough tour (?)">?</button>
                <button class="icon-btn" data-action="refresh-all" title="Re-read usage and limits now">↻</button>
              </div>
            </header>

            <div data-region="notice">${STATE.toastText ? `<div class="notice-bar" role="status">
              <span>✓ ${D.esc(STATE.toastText)}</span>
              <button class="filter-chip" data-action="clear-toast">Dismiss</button>
            </div>` : ''}</div>

            <main id="page-root" data-region="page" data-page-id="${STATE.page}" tabindex="-1">${mainContent}</main>
          </div>

          <div data-region="tray">${renderTrayPopup(accounts, metrics)}</div>
          <div data-region="dialog">${renderModal(accounts, metrics)}</div>
        </div>
      </div>
    `;

    const nextRoot = document.createElement('div');
    nextRoot.append(template.content);
    patchNode(root, Object.assign(nextRoot, { id: 'app-root' }));
    for (const [el, top, left] of scrollPositions) if (el.isConnected) { el.scrollTop = top; el.scrollLeft = left; }
    if (mountedPage !== STATE.page) {
      if (typeof disposePage === 'function') disposePage();
      disposePage = registeredPage?.mount?.(document.getElementById('page-root'), STATE);
      mountedPage = STATE.page;
    }
    root.querySelectorAll('.form-field').forEach((field) => {
      const label = field.querySelector('label'), control = field.querySelector('input[id], select[id]');
      if (label && control) label.htmlFor = control.id;
    });
    document.querySelector('.workspace').inert = !!STATE.modal;
    document.querySelector('.rail').inert = !!STATE.modal;

    // Restore scroll positions
    if (winScrollY) window.scrollTo(0, winScrollY);
    const modalElAfter = root.querySelector('.modal-card');
    if (modalElAfter && modalScrollTop) modalElAfter.scrollTop = modalScrollTop;
    const trayElAfter = root.querySelector('.tray-rings-grid');
    if (trayElAfter && trayScrollTop) trayElAfter.scrollTop = trayScrollTop;

    // Dialog focus management vs redraw focus restoration
    if (STATE.modal && prevModal !== STATE.modal) {
      const firstControl = root.querySelector('.modal-card input:not([readonly]), .modal-card select, .modal-card .tool-pick-card.active, .modal-card button:not(.icon-btn)');
      if (firstControl) firstControl.focus();
    } else if (!STATE.modal && prevModal && lastModalOpenerSelector) {
      const opener = root.querySelector(lastModalOpenerSelector);
      if (opener) opener.focus();
      lastModalOpenerSelector = null;
    } else if (activeSelector) {
      try {
        const restored = root.querySelector(activeSelector);
        if (restored) {
          restored.focus({ preventScroll: true });
          if (selStart != null && typeof restored.setSelectionRange === 'function') {
            restored.setSelectionRange(selStart, selEnd != null ? selEnd : selStart);
          }
        }
      } catch (_) {}
    }
    prevModal = STATE.modal;
  }

  // Global Event Delegation
  document.addEventListener('click', (e) => {
    const btn = e.target.closest('[data-action]');
    if (!btn) return;
    const act = btn.getAttribute('data-action');
    if (act === 'close-modal-backdrop' && e.target !== btn) return;

    const modalBeforeClick = STATE.modal;

    if (btn.disabled) return;
    switch (act) {
      case 'save-account': {
        window.HM.saveAccount?.(btn.dataset.save); break;
      }
      case 'confirm-remove':
        STATE.modal = 'remove'; break;
      case 'set-width':
        STATE.frameWidth = Number(btn.getAttribute('data-w'));
        break;
      case 'set-theme':
        STATE.theme = btn.getAttribute('data-theme');
        try { localStorage.setItem(STORAGE_THEME_KEY, STATE.theme); } catch (_) {}
        break;
      case 'set-preset':
        STATE.preset = btn.getAttribute('data-preset');
        STATE.customAccounts = [];
        STATE.removedAccountIds = [];
        break;
      case 'set-motion':
        STATE.motion = btn.getAttribute('data-motion');
        break;
      case 'simulate-redraw':
        D.BASE_ACCOUNTS.forEach((a) => { if (a.readAgoSec != null) a.readAgoSec = Math.max(1, (a.readAgoSec + 3) % 180); });
        STATE.toastText = 'Live 3-second redraw completed — selection, focus, scroll & animation phase preserved.';
        break;
      case 'nav-page':
        STATE.page = btn.getAttribute('data-page');
        STATE.pageTab = btn.dataset.tab || null;
        STATE.sessionId = btn.dataset.session || STATE.sessionId;
        STATE.projectId = btn.dataset.project || STATE.projectId;
        if (['mcp','skills','prompts','routing','apps'].includes(STATE.page)) STATE.foldSetup = true;
        break;
      case 'nav-placeholder':
        STATE.page = 'placeholder';
        STATE.placeholderTitle = btn.getAttribute('data-title') || 'Section';
        break;
      case 'toggle-pinned-projects':
        STATE.foldPinnedProjects = !STATE.foldPinnedProjects;
        break;
      case 'toggle-setup-fold':
        STATE.foldSetup = !STATE.foldSetup;
        break;
      case 'toggle-prefer-expiring':
        STATE.preferExpiring = !STATE.preferExpiring;
        break;
      case 'pick-hero-mode':
        STATE.preferExpiring = btn.getAttribute('data-expiring') === 'true';
        break;
      case 'select-lane':
        STATE.selectedAccountId = btn.getAttribute('data-account');
        break;
      case 'set-live-pc':
        STATE.livePcFilter = btn.getAttribute('data-pc');
        break;
      case 'set-win-mode':
        STATE.windowMode = btn.getAttribute('data-mode');
        break;
      case 'toggle-ring-mode':
        STATE.showUsedPct = !STATE.showUsedPct;
        break;
      case 'set-tool-filter':
        STATE.toolFilter = btn.getAttribute('data-tool');
        break;
      case 'set-pc-filter':
        STATE.computerFilter = btn.getAttribute('data-pc');
        break;
      case 'set-readout-range':
        STATE.readoutRange = btn.getAttribute('data-range');
        break;
      case 'set-clock-pc':
        STATE.yearClockPc = btn.getAttribute('data-pc');
        break;
      case 'click-attention': {
        const aid = btn.getAttribute('data-account');
        if (aid) STATE.selectedAccountId = aid;
        break;
      }
      case 'start-account': {
        const aid = btn.getAttribute('data-account');
        const account = getActiveAccounts().find((a) => a.id === aid);
        const tool = D.TOOLS.find((t) => t.id === account?.tool);
        if (STATE.preset === 'empty' || !tool?.installed || account?.needsLogin) { STATE.toastText = 'Launch unavailable: install the tool and sign in first.'; break; }
        STATE.modal = null;
        STATE.toastText = `Launch preview: ${aid} · ${STATE.startFolderPath || account.recentFolders?.[0] || 'recent folder'}`;
        break;
      }
      case 'reauth-account':
        STATE.detailAccountId = btn.dataset.account;
        STATE.signInCode = ''; STATE.signInSent = false;
        STATE.modal = 'signin';
        break;
      case 'preview-signin-page':
        STATE.toastText = 'Preview: would open the vendor sign-in page in the browser.';
        break;
      case 'send-signin-code':
        STATE.signInSent = !!STATE.signInCode?.trim();
        if (!STATE.signInSent) document.getElementById('signin-code')?.focus();
        break;
      case 'follow-session': {
        const sid = btn.getAttribute('data-session');
        STATE.followSessionId = sid || 's1';
        if (window.HM.pages.session) { STATE.page = 'session'; STATE.sessionId = sid; STATE.modal = null; }
        else STATE.modal = 'follow';
        break;
      }
      case 'open-start-folder':
        STATE.startFolderAcct = btn.getAttribute('data-account') || 'service-account-resal';
        STATE.startFolderPath = '';
        STATE.modal = 'folder';
        break;
      case 'open-add-account':
        STATE.addStep = 'form';
        STATE.modal = 'add';
        break;
      case 'open-add-account-tool': {
        STATE.addTool = btn.getAttribute('data-tool') || 'claude';
        const tObj = D.TOOLS.find((t) => t.id === STATE.addTool);
        if (tObj && !tObj.kinds[STATE.addKind].supported) {
          STATE.addKind = Object.keys(tObj.kinds).find((k) => tObj.kinds[k].supported) || 'subscription';
        }
        STATE.addStep = 'form';
        STATE.modal = 'add';
        break;
      }
      case 'set-add-tool': {
        STATE.addTool = btn.getAttribute('data-tool');
        const t = D.TOOLS.find((x) => x.id === STATE.addTool);
        if (t && !t.kinds[STATE.addKind].supported) {
          STATE.addKind = Object.keys(t.kinds).find((k) => t.kinds[k].supported) || 'subscription';
        }
        STATE.addStep = 'form';
        break;
      }
      case 'set-add-kind':
        STATE.addKind = btn.getAttribute('data-kind');
        STATE.addStep = 'form';
        break;
      case 'submit-add-account': {
        const t = D.TOOLS.find((x) => x.id === STATE.addTool) || D.TOOLS[0];
        if (!/^[a-z0-9_-]+$/i.test(STATE.addName.trim()) || !t.kinds[STATE.addKind]?.supported) {
          STATE.toastText = 'Use a profile name containing letters, numbers, hyphens or underscores.'; break;
        }
        if (STATE.addEmail && !/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(STATE.addEmail)) { STATE.toastText = 'Enter a valid expected email.'; break; }
        if (STATE.addKind === 'apikey' && !STATE.addApiKey.trim()) { STATE.toastText = 'Enter an API key.'; break; }
        if (STATE.addKind !== 'subscription' && STATE.addEndpoint && !/^https?:\/\//.test(STATE.addEndpoint)) { STATE.toastText = 'Enter an http or https endpoint.'; break; }
        if (STATE.addKind === 'routed' && !D.ROUTER_ROUTES.some((r) => r.id === STATE.addRoute)) { STATE.toastText = 'Choose an available route.'; break; }
        if (STATE.addStep === 'oauth-code' && !STATE.oauthCode.trim()) { STATE.toastText = 'Paste a code or callback URL for the sign-in preview.'; break; }
        const clean = (STATE.addName || 'new-acct').trim().replace(/[^a-z0-9-_]/gi, '').toLowerCase() || 'profile';
        const newId = clean.startsWith(t.id) ? clean : `${t.id}-${clean}`;
        if (getActiveAccounts().some((a) => a.id === newId || a.folder === `${t.folderPrefix}${clean}`)) { STATE.toastText = 'That profile already exists. Choose another name.'; break; }
        if (STATE.addKind === 'subscription' && t.id === 'claude' && STATE.addStep === 'form') {
          STATE.addStep = 'oauth-code';
          break;
        }
        // Newly added accounts start unread (measured: false, windows: [], —) and stay in current preset
        STATE.customAccounts.push({
          id: newId,
          name: newId,
          isMain: false, needsLogin: STATE.addKind === 'subscription',
          tool: t.id,
          kind: STATE.addKind,
          plan: STATE.addKind === 'subscription' ? 'Subscription' : STATE.addKind === 'apikey' ? 'API Key' : `Routed · ${STATE.addRoute}`,
          email: STATE.addKind === 'subscription' ? (STATE.addEmail || 'pending sign-in') : STATE.addKind === 'apikey' ? STATE.addEndpoint : `router://127.0.0.1:8787/${STATE.addRoute}`,
          folder: `${t.folderPrefix}${clean}`,
          added: 'Just now',
          readAgoSec: null,
          measured: false,
          advice: 'Limits not read (—) · sign in or refresh to read windows.',
          adviceTone: 'neutral',
          openSessions: 0,
          computers: ['developmentpc'],
          firstReplySec: null,
          tokens7d: 0,
          cost7d: 0,
          spark7d: [0, 0, 0, 0, 0, 0, 0],
          windows: [],
          modelsBreakdown: [],
          recentFolders: ['D:\\Projects\\abushanab-net\\HarnessManager']
        });
        STATE.addApiKey = ''; STATE.oauthCode = '';
        STATE.selectedAccountId = newId;
        STATE.modal = null;
        STATE.toastText = `Created preview profile ${newId} (${t.folderPrefix}${clean}) — limits not read (—) until first refresh.`;
        break;
      }
      case 'open-account-detail':
        STATE.detailAccountId = btn.getAttribute('data-account') || 'service-account-resal';
        STATE.detailTab = btn.dataset.tab || 'overview';
        STATE.modal = null;
        STATE.trayOpen = false;
        STATE.page = 'account';
        break;
      case 'set-detail-tab':
        STATE.detailTab = btn.getAttribute('data-tab');
        break;
      case 'open-account-menu':
        e.stopPropagation();
        STATE.detailAccountId = btn.getAttribute('data-account') || 'service-account-resal';
        STATE.modal = 'acctmenu';
        break;
      case 'open-request-log':
        STATE.modal = 'reqlog';
        break;
      case 'open-modal':
        STATE.modal = btn.getAttribute('data-modal');
        break;
      case 'close-modal':
      case 'close-modal-backdrop':
        STATE.modal = null;
        break;
      case 'toggle-tray':
        STATE.trayOpen = !STATE.trayOpen;
        break;
      case 'refresh-all': {
        const accts = getActiveAccounts();
        D.BASE_ACCOUNTS.forEach((a) => { if (a.readAgoSec != null) a.readAgoSec = 1; });
        STATE.toastText = `Refresh preview: ${accts.length} accounts; no live vendor read performed.`;
        break;
      }
      case 'refresh-single-account': {
        e.stopPropagation();
        const aid = btn.getAttribute('data-account');
        const target = D.BASE_ACCOUNTS.find((a) => a.id === aid) || STATE.customAccounts.find((a) => a.id === aid);
        if (target) target.readAgoSec = 1;
        STATE.toastText = `Refresh preview: ${aid}; no live vendor read performed.`;
        break;
      }
      case 'clear-toast':
        STATE.toastText = null;
        break;
      case 'toast-and-close':
        STATE.toastText = 'Preview: ' + btn.getAttribute('data-msg');
        STATE.modal = null;
        break;
      case 'remove-account': {
        const aid = btn.getAttribute('data-account');
        if (aid && !STATE.removedAccountIds.includes(aid)) {
          STATE.removedAccountIds.push(aid);
        }
        STATE.modal = null;
        if (STATE.page === 'account') STATE.page = 'accounts';
        STATE.toastText = `Hidden ${aid} from active accounts (${getActiveAccounts().length} remaining).`;
        break;
      }
      case 'set-acct-range':
        STATE.acctRange = btn.getAttribute('data-range');
        break;
      case 'set-acct-tool':
        STATE.acctToolFilter = btn.getAttribute('data-tool');
        break;
      case 'toggle-fold-daily':
        STATE.foldDaily = !STATE.foldDaily;
        break;
      case 'toggle-fold-models':
        STATE.foldModels = !STATE.foldModels;
        break;
      case 'toggle-tool-group': {
        const tid = btn.getAttribute('data-tool');
        STATE.collapsedTools[tid] = !STATE.collapsedTools[tid];
        break;
      }
      case 'export-csv': {
        const accts = getActiveAccounts();
        triggerCsvDownload(accts);
        STATE.toastText = `Exported harness-accounts-${STATE.acctRange}.csv (${accts.length} rows, API list prices).`;
        break;
      }
      case 'set-sync-choice':
        STATE.syncChoice = btn.getAttribute('data-sync');
        break;
      default: return;
    }

    if (!modalBeforeClick && STATE.modal) {
      lastModalOpenerSelector = buildElementSelector(btn);
    }
    renderApp();
  });

  // Keyboard navigation: Ctrl+K focuses search, Tab traps inside modal, Enter/Space activates [tabindex="0"][data-action], Escape closes modal or resets selection
  document.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      const inp = document.getElementById('global-search');
      if (inp) inp.focus();
      return;
    }

    if (e.key === 'Tab' && STATE.modal) {
      const card = document.querySelector('.modal-card');
      if (card) {
        const focusables = Array.from(card.querySelectorAll('button:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex="0"]'));
        if (focusables.length > 0) {
          const first = focusables[0];
          const last = focusables[focusables.length - 1];
          if (e.shiftKey && document.activeElement === first) {
            e.preventDefault();
            last.focus();
          } else if (!e.shiftKey && document.activeElement === last) {
            e.preventDefault();
            first.focus();
          }
        }
      }
      return;
    }

    if (e.key === 'Enter' && e.target.id === 'global-search' && !['overview','accounts','sessions','mcp','skills'].includes(STATE.page)) {
      STATE.page = 'sessions'; renderApp(); return;
    }

    if (e.key === 'Enter' || e.key === ' ') {
      const target = e.target;
      if (target && target.tagName !== 'BUTTON' && target.tagName !== 'INPUT' && target.tagName !== 'SELECT' && target.hasAttribute('data-action')) {
        e.preventDefault();
        target.click();
        return;
      }
    }

    if (e.key === 'Escape') {
      if (STATE.modal) {
        STATE.modal = null;
        renderApp();
      } else if (STATE.trayOpen) {
        STATE.trayOpen = false;
        renderApp();
      } else if (STATE.selectedAccountId !== 'codex-main') {
        STATE.selectedAccountId = 'codex-main';
        renderApp();
      }
    }
  });

  window.HM_APP = {
    STATE,
    renderApp,
    getActiveAccounts,
    // Updates state without destroying focused input on keystrokes
    updateField(k, v) {
      STATE[k] = v;
      if (k === 'startFolderAcct') STATE.startFolderPath = '';
      if (k === 'startFolderAcct' || k === 'startSkipPerms') {
        renderApp();
        return;
      }
      const { toolObj, safeName, cliCmd } = computeAddCliCmd();
      const folderEl = document.querySelector('[data-bind="add-folder-preview"]');
      if (folderEl) folderEl.textContent = `${toolObj.folderPrefix}${safeName}`;
      const cliEl = document.querySelector('[data-bind="add-cli-preview"]');
      if (cliEl) cliEl.textContent = cliCmd;
    },
    onSearch(val) {
      STATE.searchQuery = val;
      renderApp();
    }
  };

  Object.assign(window.HM, {
    state: STATE,
    titles: { sessions:'Sessions', session:'Session', projects:'Projects', mcp:'MCP servers', skills:'Skills', prompts:'System prompts', routing:'API keys & routing', apps:'Apps', cli:'CLI reference', settings:'Settings', account:'Account' },
    navigate(id, fields = {}) { Object.assign(STATE, fields, {page:id, modal:null}); renderApp(); },
    redraw: renderApp,
    esc: D.esc
  });
  window.addEventListener('DOMContentLoaded', renderApp);
})();

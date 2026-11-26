/* Calibre · S11 Sessions, S12 Session detail, S13 Stop confirmation (and its two honest outcomes). */

const RUNNING = [
  // title, project, account, tool, computer, age, state
  ['Account limit re-login notifications', 'HarnessManager-rc-chat', 'sabushanab-resal', 'claude', 'DEVELOPMENTPC', '3 h 11 m', 'needs'],
  ['Use /speckit-workflow-continue 29. Read D:\\Projects\\Bunyan\\Issue…', 'Bunyan', 'samykabu-teksa', 'claude', 'LAPTOP', '1 h 19 m', 'needs'],
  ['Delegated task', 'HarnessManager-rc-chat', 'codex-main', 'codex', 'DEVELOPMENTPC', '38 m', 'working'],
  ['Delegated task', 'subscription-entitlements', 'claude-Resal-Dev01', 'claude', 'DEVELOPMENTPC', '33 m', 'working'],
  ['Delegated task', 'subscription-entitlements', 'claude-Resal-Dev01', 'claude', 'DEVELOPMENTPC', '1 h 39 m', 'working'],
  ['Delegated task', 'wt-mirbad-lan-models', 'codex-main', 'codex', 'LAPTOP', '1 h 37 m', 'working'],
  ['mirbadllm-agent installer on developmentpc', 'sabus', 'claude-main', 'claude', 'DEVELOPMENTPC', '2 h 15 m', 'idle'],
];
const EARLIER = [
  ['Mirbad GGUF reader unknown types', 'wt-mirbad-lan-models', 'samykabu-personal', 'claude', 'DEVELOPMENTPC', '5 h ago', 'ended'],
  ['VSCode picker omits lnkling-2-mini-mac', 'sabus', 'claude-main', 'claude', 'DEVELOPMENTPC', '5 h ago', 'ended'],
  ['app shows "Filter accounts/Run a commandCtrl K7 live3 computersl…', 'HarnessManager-rc-chat', 'codex-main', 'codex', 'DEVELOPMENTPC', '5 h ago', 'ended'],
];
const ST = { needs: ['st-needs', 'Needs you'], working: ['st-work', 'Working'], idle: ['st-idle', 'Idle'], ended: ['st-end', 'Ended'] };
const sRow = ([t, p, a, tl, c, age, s], i) => `<button class="ss-row" data-to="${i === 0 ? 'session' : i === 5 ? 'session-nc' : 'session'}">
    <span class="ss-tool">${tool(tl)}</span>
    <span class="ss-main"><b>${t}</b><small>${p} · ${a}</small></span>
    <span class="ss-st"><span class="st ${ST[s][0]}">${s === 'needs' ? '<span class="pulse" aria-hidden="true"></span>' : '<i aria-hidden="true"></i>'}${ST[s][1]}</span><small>${c === 'DEVELOPMENTPC' ? 'DEV PC' : c} · ${age}</small></span>
  </button>`;

function sessions(v = '') {
  const run = v === 'quiet' ? RUNNING.map(r => r[6] === 'needs' ? [...r.slice(0, 6), 'working'] : r) : RUNNING;
  return `${chrome()}<div class="scroll">
    ${largeTitle('Sessions', '', v === 'quiet' ? 0 : 2)}
    <div class="search">${ic('magnifying-glass')}<span>Search titles, projects, accounts</span></div>
    <div class="filters" role="toolbar" aria-label="Filters">
      <button class="chip chip-ink">${ic('check')}Running</button><button class="chip">Project${ic('caret-down')}</button><button class="chip">Account${ic('caret-down')}</button><button class="chip">Computer${ic('caret-down')}</button><button class="chip">Tool${ic('caret-down')}</button></div>
    <h3 class="sub-h pad">Running <span>7 on 2 computers</span></h3>
    <div class="card rows ss">${run.map(sRow).join('')}</div>
    <h3 class="sub-h pad">Earlier today</h3>
    <div class="card rows ss">${EARLIER.map((r, i) => sRow(r, 9 + i)).join('')}</div>
    <p class="fine">MINI is asleep: its sessions show as of 2 days ago, read-only.</p>
  </div>${tabbar('Sessions')}`;
}

/* v: '' (waiting for you), 'stop' (S13 sheet open), 'stopped' / 'unknown' (the two outcomes), 'nc' (no Control on this session) */
function session(v = '') {
  if (v === 'nc') return sessionNoControl();
  const sheet = v === 'stop', after = v === 'stopped' || v === 'unknown';
  return `${chrome()}<div class="scroll" data-bottom style="padding-bottom:${after ? 210 : 150}px">
    ${navbar('Sessions', '', 'dots-three')}
    <header class="sd-head"><h1>Account limit re‑login notifications</h1>
      <p class="sd-sub">${tool('claude')}sabushanab-resal · DEVELOPMENTPC</p>
      <div class="sd-state">${after ? `<span class="st st-idle"><i></i>Idle at the prompt</span>` : `<span class="chip chip-signal chip-xs"><span class="pulse" aria-hidden="true"></span>Waiting for you</span>`}
        <span class="cap">3 h 11 m</span>
        <button class="chip chip-tap follow" data-toggle aria-checked="${after ? 'true' : 'false'}" aria-label="Follow on the Lock Screen">${ic('broadcast')}<span>Follow</span></button></div>
    </header>
    <div class="tx">
      <div class="msg you"><span class="k">You · 18:11</span><p>When an account's login expires, show a re-login banner on its card and send a notification.</p></div>
      <div class="msg ai"><span class="k">${tool('claude')}Claude · 18:12</span><p>I'll add the banner to the accounts pane, and send a notification when the limits read fails with an expired login.</p></div>
      <div class="steps">
        <div class="step open">${ic('pencil-simple-line')}<span>Edited 3 files</span><em class="diff"><ins>+148</ins> <del>−12</del></em>${ic('caret-up')}</div>
        <ul class="files"><li><span class="mono">app/ui/accounts.js</span><em class="diff"><ins>+96</ins> <del>−8</del></em></li><li><span class="mono">crates/core/src/harness/claude.rs</span><em class="diff"><ins>+41</ins> <del>−4</del></em></li><li><span class="mono">app/src-tauri/src/state.rs</span><em class="diff"><ins>+11</ins> <del>−0</del></em></li></ul>
        <div class="step">${ic('check-circle')}<span>Ran <span class="mono">cargo test</span> · 36 passed</span>${ic('caret-down')}</div>
        <div class="step warn">${ic('warning-circle')}<span>Ran <span class="mono">npm run lint</span> · 1 warning, fixed</span>${ic('caret-down')}</div>
      </div>
      <div class="msg ai"><p>The re-login banner and its notification are in. I'll run the unit tests to check nothing else broke.</p></div>
      ${after ? `<div class="msg sys">${ic(v === 'stopped' ? 'stop-circle' : 'question')}<p>${v === 'stopped' ? 'Turn stopped from your phone at 21:24:10. The tests did not run.' : 'Stop sent at 21:24:10. No receipt yet.'}</p></div>`
      : `<div class="inline-req card" data-ireq>
        <div class="ir-h"><span class="pulse" aria-hidden="true"></span><b>Permission · run a command</b><span class="clock">${countdown(28, 252, 300, 'p1')}<b data-left="p1" data-start="252">4:12</b></span></div>
        <code>npm run test -- --watch=false</code>
        <div class="row2"><button class="btn btn-secondary btn-sm" data-ideny>Deny</button><button class="btn btn-primary btn-sm" data-iapprove>${ic('scan')}Approve once</button></div>
      </div>`}
    </div>
  </div>
  ${after ? `<div class="toast ${v === 'stopped' ? 'ok' : 'unk'}" role="status">
      <span class="t-i">${v === 'stopped' ? ic('check') : ic('arrows-clockwise')}</span>
      <div><b>${v === 'stopped' ? 'Interrupt delivered' : 'Delivery unknown'}</b><p>${v === 'stopped' ? 'DEVELOPMENTPC confirmed Ctrl+C at 21:24:10 · direct, 120 ms.' : 'The network dropped before a receipt. Reading the session again; Stop is not sent twice.'}</p></div>
    </div>` : ''}
  <div class="composer">
    <button class="stop-btn" data-to="stop" aria-label="Stop this turn">${ic('stop', 1)}</button>
    <div class="reply"><span>Reply to the terminal…</span></div>
    <button class="send" aria-label="Send" disabled>${ic('arrow-up')}</button>
  </div>
  ${sheet ? `<div class="scrim" data-close></div>
  <section class="sheet stop-sheet" role="dialog" aria-modal="true" aria-label="Stop this turn?">
    <span class="grabber" aria-hidden="true"></span>
    <span class="ss-ic">${ic('stop', 1)}</span>
    <h3>Stop this turn?</h3>
    <p class="ss-p">Sends Ctrl+C once to “Account limit re-login notifications” on DEVELOPMENTPC. The session stays open and you can reply after.</p>
    <p class="ss-note">${ic('info')}<span>Pressed twice at an idle prompt, Ctrl+C can close Claude Code, so the phone only ever sends it once.</span></p>
    <button class="btn btn-primary wide" data-to="stopped">${ic('stop', 1)}Stop the turn</button>
    <button class="btn btn-quiet wide" data-back data-close style="margin-top:8px">Cancel</button>
  </section>` : ''}`;
}
function sessionNoControl() {
  return `${chrome()}<div class="scroll" style="padding-bottom:170px">
    ${navbar('Sessions', '', 'dots-three')}
    <header class="sd-head"><h1>Delegated task</h1>
      <p class="sd-sub">${tool('codex')}codex-main · LAPTOP, through your hub</p>
      <div class="sd-state"><span class="st st-work"><i></i>Working</span><span class="cap">1 h 37 m · wt-mirbad-lan-models</span>
        <button class="chip chip-tap follow" data-toggle aria-checked="false">${ic('broadcast')}<span>Follow</span></button></div>
    </header>
    <div class="tx">
      <div class="msg you"><span class="k">You · 19:45</span><p>Find why the LAN model list drops GGUF files with unknown tensor types, and fix the reader.</p></div>
      <div class="msg ai"><span class="k">${tool('codex')}Codex · 19:46</span><p>Reading the GGUF header parser first, then I'll add the missing type ids with a test for each.</p></div>
      <div class="steps">
        <div class="step">${ic('magnifying-glass')}<span>Read 14 files</span>${ic('caret-down')}</div>
        <div class="step">${ic('pencil-simple-line')}<span>Edited 2 files</span><em class="diff"><ins>+63</ins> <del>−9</del></em>${ic('caret-down')}</div>
        <div class="step">${ic('circle-notch')}<span>Running <span class="mono">cargo test -p mirbad</span></span><span class="cap">now</span></div>
      </div>
    </div>
  </div>
  <div class="composer off">
    <p class="why">${ic('lock-simple')}<span>Reply and Stop need a session started in Harness Manager's own terminal. This one runs in Windows Terminal on LAPTOP.</span></p>
    <div class="row-off"><button class="stop-btn" disabled aria-label="Stop, unavailable">${ic('stop', 1)}</button><div class="reply"><span>Reply unavailable for this session</span></div></div>
  </div>`;
}

Object.assign(SCREENS, { sessions, session, stop: () => session('stop'), stopped: () => session('stopped'), 'session-nc': () => session('nc') });

/* inline answer inside the transcript: Face ID, then the card closes into a one-line receipt with the tick landing */
document.addEventListener('click', e => {
  const b = e.target.closest('[data-iapprove], [data-ideny]'); if (!b) return;
  const card = b.closest('[data-ireq]'), ok = b.hasAttribute('data-iapprove');
  const done = () => { card.classList.add('resolved'); card.innerHTML = `<span class="ir-done">${ic(ok ? 'check' : 'x')}</span><div><b>${ok ? 'Approved once' : 'Denied'}</b><p>From this phone at 21:22:48 · DEVELOPMENTPC confirmed it</p></div>`; };
  if (ok && !RM()) { b.lastChild.textContent = 'Face ID'; setTimeout(done, 600); } else done();
});

/* a transcript opens at its latest message, where a waiting request is */
const toBottom = root => root.querySelectorAll('.scroll[data-bottom]').forEach(sc => { sc.scrollTop = sc.scrollHeight; });
document.addEventListener('calibre:mount', () => toBottom(document));
document.addEventListener('proto:nav', e => toBottom(e.detail.ph));

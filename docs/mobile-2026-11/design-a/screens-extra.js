/* Calibre · review 3 additions: Home and Permission at the largest accessibility text size (iOS AX5), the Sessions filter sheet,
   the Limit-low threshold sheet. AX5 sizes are Apple's: Large Title 60, Title 1 58, Title 2 56, Headline/Body 53, Subheadline 49,
   Footnote 44, Caption 40. Bars (navigation, tabs) keep their size and use the Large Content Viewer on long press, as iOS does. */

function homeAx() {
  return `${chrome()}<div class="scroll ax">
    <header class="ax-lt"><h1>Home</h1></header>
    <button class="ax-needs" data-to="requests"><span class="pulse" aria-hidden="true"></span><span><b>2 waiting</b><span class="tm" data-left="q1" data-start="41" data-soon>0:41</span> left</span></button>
    <article class="card ax-card">
      <h2 class="ax-h">Needs you</h2>
      <button class="ax-req" data-to="permission"><span class="ax-k">Permission</span><code>npm run test -- --watch=false</code>
        <span class="ax-time"><b data-left="p1" data-start="252">4:12</b> left</span><span class="ax-m">DEVELOPMENTPC</span></button>
      <button class="ax-req" data-req="q1" data-to="question"><span class="ax-k">Question</span><span class="ax-q">Which branch should I base this on?</span>
        <span class="ax-time"><b data-left="q1" data-start="41">0:41</b> left</span><span class="ax-m" data-qmeta>LAPTOP</span></button>
    </article>
    <article class="card ax-card">
      <h2 class="ax-h">Use next</h2>
      <div class="ax-dial">${dial(220, .64, { w: 12, label: '64% of the week left' })}<b class="serif-num">64%</b></div>
      <p class="ax-acct">claude-Resal-Dev01</p>
      <p class="ax-why">64% erased Sat 18:58</p>
    </article>
    <article class="card ax-card"><h2 class="ax-h">Running</h2><p class="ax-why">7 sessions</p><p class="ax-m">DEVELOPMENTPC 5, LAPTOP 2</p></article>
    <article class="card ax-card"><h2 class="ax-h">Next reset</h2><p class="ax-why">1 h 48 m</p><p class="ax-m">sabushanab-resal, 5-hour</p></article>
    <button class="card ax-card ax-game" data-to="progress"><span class="ax-h">Planner</span><span class="ax-m">1,240 of 1,500 XP, 12-day streak</span></button>
  </div>${tabbar('Home')}`;
}

function permissionAx() {
  return `${chrome()}<div class="scroll ax" style="padding-bottom:330px">
    ${navbar('Home', 'Permission')}
    <div class="ax-pad">
      <p class="ax-k">Claude Code · sabushanab-resal</p>
      <h2 class="ax-t1">Wants to run a command</h2>
      <code class="ax-cmd">npm run test -- --watch=false</code>
      <p class="ax-k">Session</p><p class="ax-b">Account limit re‑login notifications</p>
      <p class="ax-k">Claude, just before asking</p><p class="ax-b ax-quote">“I'll run the unit tests to check nothing else broke.”</p>
      <p class="ax-k">On the computer</p><p class="ax-b">DEVELOPMENTPC</p>
      <p class="ax-k">In the folder</p><p class="ax-b ax-path">D:\\<wbr>Projects\\<wbr>abushanab-net\\<wbr>HarnessManager-rc-chat</p>
    </div>
  </div>
  <div class="actions ax-actions">
    <p class="ax-time"><b class="serif-num" data-left="p2" data-start="252">4:12</b> left</p>
    <button class="btn btn-primary wide" data-approve>Approve once</button>
    <button class="btn btn-secondary wide" data-deny>Deny</button>
  </div>
  <section class="receipt" aria-live="polite"><div class="seal">${dial(96, 1, { w: 4, cls: 'lume', ticks: 60 })}${ic('check')}</div><h2 data-rt></h2><p data-rp></p>
    <button class="btn btn-primary done" data-reset data-to="home">Back to Home</button></section>`;
}

/* Sessions › filters: one sheet for every filter chip, opened at the medium detent */
function filterSheet() {
  const chip = (t, on) => `<button class="chip${on ? ' chip-ink' : ''}">${on ? ic('check') : ''}${t}</button>`;
  const row = (i, t, sub, on) => `<div class="li">${ic(i)}<span><b>${t}</b>${sub ? `<small>${sub}</small>` : ''}</span>${on ? `<span class="on">${ic('check', 1)}</span>` : '<span></span>'}</div>`;
  return `${sessions().replace(tabbar('Sessions'), tabbar('Sessions'))}
  <div class="scrim" data-close></div>
  <section class="sheet fsheet" role="dialog" aria-modal="true" aria-label="Filter sessions">
    <span class="grabber" aria-hidden="true"></span>
    <div class="fs-bar"><button class="nb-done" data-close style="font-weight:500">Cancel</button><b>Filter</b><button class="nb-done" data-close>Reset</button></div>
    <div class="fs-body">
      <h4>Show</h4><div class="tabs" role="tablist"><button role="tab" aria-selected="true">Running</button><button role="tab" aria-selected="false">Today</button><button role="tab" aria-selected="false">All</button></div>
      <h4>Project</h4><div class="fs-chips">${chip('HarnessManager-rc-chat', 1)}${chip('Bunyan')}${chip('subscription-entitlements')}${chip('wt-mirbad-lan-models')}${chip('sabus')}${chip('7 more')}</div>
      <h4>Computer</h4><div class="card list">${row('desktop-tower', 'DEVELOPMENTPC', '5 running', 1)}${row('laptop', 'LAPTOP', '2 running', 1)}${row('moon', 'MINI', 'asleep, read-only', 0)}</div>
      <h4>Tool</h4><div class="fs-chips">${chip('Claude Code', 1)}${chip('Codex', 1)}${chip('OpenCode')}${chip('Antigravity')}</div>
      <h4>Account</h4><div class="fs-chips">${chip('Any account', 1)}${chip('sabushanab-resal')}${chip('codex-main')}${chip('claude-Resal-Dev01')}</div>
    </div>
    <div class="fs-foot"><button class="btn btn-primary wide" data-close>Show 2 sessions</button></div>
  </section>`;
}

/* Notifications › Limit low: the threshold as a dial, presets, and the accounts it would alert about now */
function thresholdSheet() {
  const S = 168, c = S / 2, r = 66;
  const [mx, my] = pt(c, c, r, .2 * 360);
  return `${notifs()}
  <div class="scrim" data-close></div>
  <section class="sheet tsheet" role="dialog" aria-modal="true" aria-label="Limit low">
    <span class="grabber" aria-hidden="true"></span>
    <div class="fs-bar"><button class="nb-done" data-close style="font-weight:500">Cancel</button><b>Limit low</b><button class="nb-done" data-close>Done</button></div>
    <div class="ts-top">
      <div class="ts-dial"><svg class="dial" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="Alert below 20% of room left">
        ${tickRing(c, c, c - 1, 50, 3, 5, 7)}${ringArc(c, c, r, 10, .2, 'amber')}<circle cx="${mx.toFixed(1)}" cy="${my.toFixed(1)}" r="9" class="ts-knob"/></svg>
        <div class="c"><b class="serif-num">20%</b><span>room left</span></div></div>
      <div><p class="ts-k">Alert when an account falls below</p>
        <div class="sizes ts-pre" role="radiogroup" aria-label="Presets">${[10, 20, 30, 50].map(v => `<button role="radio" aria-checked="${v === 20}" data-pick>${v}%</button>`).join('')}</div>
        <p class="ts-k" style="margin-top:14px">In which window</p>
        <div class="sizes ts-pre" role="radiogroup" aria-label="Window"><button role="radio" aria-checked="true" data-pick>Week</button><button role="radio" aria-checked="false" data-pick>5-hour</button><button role="radio" aria-checked="false" data-pick>Both</button></div></div>
    </div>
    <p class="ts-now">${ic('warning')}<span><b>claude-main is already below:</b> 11% of its week, resets Fri 14:58. You would get one alert now.</span></p>
    <h4 class="ts-h">Per account</h4>
    <div class="card list">
      <div class="li">${tool('claude')}<span><b>claude-Resal-Dev01</b><small>custom</small></span><span class="chip chip-xs">30%</span></div>
      <div class="li">${tool('claude')}<span><b>claude-main and 5 more</b><small>use the default</small></span><span class="chip chip-xs">20%</span></div>
      <div class="li">${tool('codex')}<span><b>codex-main</b><small>alerts off</small></span><span class="chip chip-xs">Off</span></div>
    </div>
    <p class="fine" style="margin:10px 2px 0;text-align:left">One alert per window, never repeated. opencode-main and Claude-Resal-API report no limit window and are skipped. Quiet hours apply.</p>
  </section>`;
}

Object.assign(SCREENS, { 'home-ax': homeAx, 'permission-ax': permissionAx, filter: filterSheet, threshold: thresholdSheet });

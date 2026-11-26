/* Calibre screens. Data from ../brief/data.json (snapshot 25 Nov 2026), "now" = Wed 25 Nov 21:22.
   Invented for the mockup: request texts and timers, running count, XP, level, streak, badges, recap. */

const ic = (n, fill) => `<i class="${fill ? 'ph-fill' : 'ph'} ph-${n}" aria-hidden="true"></i>`;
const tool = t => `<img src="icons/${t}.svg" alt="" width="16" height="16">`;
const RM = () => document.body.classList.contains('rm') || matchMedia('(prefers-reduced-motion: reduce)').matches;

/* ---------- dial primitives (SVG geometry, the one visual language) ---------- */
const pt = (cx, cy, r, deg) => { const a = (deg - 90) * Math.PI / 180; return [cx + r * Math.cos(a), cy + r * Math.sin(a)]; };
function tickRing(cx, cy, r, n, len, majorEvery, majorLen, from = 0, span = 360) {
  let s = '';
  const steps = span === 360 ? n : n - 1;
  for (let i = 0; i < n; i++) {
    const deg = from + span * i / steps, major = majorEvery && i % majorEvery === 0, L = major ? majorLen : len;
    const [x1, y1] = pt(cx, cy, r, deg), [x2, y2] = pt(cx, cy, r - L, deg);
    s += `<line class="tick${major ? ' major' : ''}" x1="${x1.toFixed(2)}" y1="${y1.toFixed(2)}" x2="${x2.toFixed(2)}" y2="${y2.toFixed(2)}"/>`;
  }
  return s;
}
/* full ring: track + arc that fills on open. frac 0..1 of the circle */
function ringArc(cx, cy, r, w, frac, cls = '', extra = '') {
  const C = 2 * Math.PI * r;
  return `<circle class="track" cx="${cx}" cy="${cy}" r="${r}" stroke-width="${w}"/>
    <circle class="arc ${cls}" ${extra} cx="${cx}" cy="${cy}" r="${r}" stroke-width="${w}" transform="rotate(-90 ${cx} ${cy})"
      style="--c:${C.toFixed(2)};--v:${(C * (1 - frac)).toFixed(2)}"/>`;
}
function dial(size, frac, { w = 8, ticks = 60, cls = '', label = '', live = '' } = {}) {
  const c = size / 2, r = c - w / 2 - (ticks ? 9 : 1);
  return `<svg class="dial" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}" role="img" aria-label="${label}">
    ${ticks ? tickRing(c, c, c - 1, ticks, 3, 5, 6) : ''}${ringArc(c, c, r, w, frac, cls, live)}</svg>`;
}
/* countdown mini dial: remaining time drains from the arc in real time */
function countdown(size, left, total, id) {
  const c = size / 2, r = c - 4, C = 2 * Math.PI * r;
  return `<svg class="dial" width="${size}" height="${size}" viewBox="0 0 ${size} ${size}" aria-hidden="true">
    ${tickRing(c, c, c, 12, 2, 3, 2)}
    <circle class="track" cx="${c}" cy="${c}" r="${r - 3}" stroke-width="3"/>
    <circle class="arc live" data-cd="${id}" data-total="${total}" cx="${c}" cy="${c}" r="${r - 3}" stroke-width="3" transform="rotate(-90 ${c} ${c})"
      style="--c:${(2 * Math.PI * (r - 3)).toFixed(2)};--v:${(2 * Math.PI * (r - 3) * (1 - left / total)).toFixed(2)}"/></svg>`;
}
const mmss = s => `${Math.floor(s / 60)}:${String(Math.max(0, s) % 60).padStart(2, '0')}`;

const chrome = (time = '21:22') => `<div class="island"></div>
  <div class="status"><span>${time}</span><span class="r">${ic('cell-signal-full', 1)}${ic('wifi-high', 1)}${ic('battery-full', 1)}</span></div>
  <div class="homebar"></div>`;
const TABS = [['Home', 'house', 'home'], ['Sessions', 'terminal-window', 'sessions'], ['Accounts', 'gauge', 'accounts'], ['Computers', 'desktop', 'computers'], ['You', 'user-circle', 'you']];
function tabbar(active) {
  return `<nav class="tabbar" aria-label="Tabs">${TABS.map(([n, i, k]) =>
    `<a href="#" data-to="${k}" data-tab ${n === active ? 'aria-current="page"' : ''}>${ic(i, n === active)}<span>${n}</span></a>`).join('')}</nav>`;
}
/* pushed-screen bar: back, compact title, a right slot (the Needs-you pill or an action) */
const navbar = (back, title = '', act = '') => `<div class="navbar"><button class="back" data-back aria-label="Back to ${back}">${ic('caret-left')}${back}</button><span class="ttl">${title}</span>${
  act.startsWith('<') ? `<span class="nb-r">${act}</span>` : act ? `<button class="act" aria-label="More">${ic(act)}</button>` : '<span></span>'}</div>`;
/* Needs you, reachable from every tab: a pill in the large-title bar with the count and the soonest countdown.
   With nothing waiting it shrinks to a quiet tray button that still opens the inbox. */
const needsPill = (n = 2) => n ? `<button class="needs" data-to="requests" aria-label="${n} requests waiting. Open Requests.">
    <span class="pulse" aria-hidden="true"></span><b data-needs>${n}</b><span class="needs-t" data-left="q1" data-start="41" data-soon>0:41</span></button>`
  : `<button class="req-btn" data-to="requests" aria-label="Requests, none waiting">${ic('tray')}</button>`;
const largeTitle = (title, date = '', n = 2, extra = '') => `<header class="lt"><div>${date ? `<p class="date">${date}</p>` : ''}<h1>${title}</h1></div><div class="lt-r">${extra}${needsPill(n)}</div></header>`;

/* ---------- S7 Home ---------- */
const days30 = [0.63, 2.02, 1.98, 2.63, 3.7, 2.16, 2.6, 1.15, 1.6, 2.26, 2.62, 2.27, 1.72, 2.03, 2.36, 1.95, 1.49, 0.66, 1.26, 1.76, 1.26, 3.18, 2.57, 1.69, 1.21, 3.44, 3.5, 1.41, 4.53, 2.23];
/* v: '' live, 'asleep' (a computer sleeps), 'hub' (hub unreachable), 'offline' (phone offline). The last three are S22. */
const BANNERS = {
  asleep: ['moon', 'DEVELOPMENTPC is asleep', 'Showing what it last sent, 18 m ago. Its request waits there until it wakes.', ''],
  hub: ['cloud-slash', 'Your hub is not answering', 'Away from home, so readings are 6 m old. Retrying in 12 s.', 'Details'],
  offline: ['wifi-slash', 'No connection', "Everything here is from this phone's cache, 41 m ago.", ''],
};
const banner = v => { const [i, t, p, act] = BANNERS[v];
  return `<div class="banner b-${v}" role="status">${ic(i)}<div><b>${t}</b><p>${p}</p></div>${act ? `<button class="chip chip-tap">${act}</button>` : ''}</div>`; };
function home(v = '') {
  const max = Math.max(...days30);
  const reserve = 1240 / 1500, stale = !!v, age = { asleep: '18 m', hub: '6 m', offline: '41 m' }[v];
  const unknownReq = v === 'hub' || v === 'offline';
  const permRow = `<button class="req-row${v === 'asleep' ? ' gone' : ''}" data-to="permission" ${v === 'asleep' ? 'disabled' : ''}>
          <span class="ico">${ic('terminal')}</span>
          <span style="min-width:0"><div class="what">Permission · run a command</div><div class="main mono">npm run test -- --watch=false</div>
            <div class="meta">${v === 'asleep' ? 'Waits on DEVELOPMENTPC, asleep' : 'DEVELOPMENTPC · HarnessManager-rc-chat'}</div></span>
          <span class="clock">${v === 'asleep' ? `<span class="zz">${ic('moon')}</span><b>asleep</b>` : `${countdown(36, 252, 300, 'p1')}<b data-left="p1" data-start="252">4:12</b>`}</span>
        </button>`;
  const qRow = `<button class="req-row" data-req="q1" data-to="question">
          <span class="ico">${ic('chat-circle-text')}</span>
          <span style="min-width:0"><div class="what">Question</div><div class="main">Which branch should I base this on?</div>
            <div class="meta" data-qmeta>LAPTOP · Bunyan</div></span>
          <span class="clock">${countdown(36, 41, 60, 'q1')}<b data-left="q1" data-start="41">0:41</b></span>
        </button>`;
  const needs = unknownReq
    ? `<h2 class="w-title">Needs you <span class="serif-num" style="font-size:15px;color:var(--ink)">—</span></h2>
       <p class="unknown">${ic('question')}<span>Unknown right now. Requests still wait in each terminal and arrive here when ${v === 'hub' ? 'the hub answers' : 'this phone is back online'}.</span></p>`
    : `<h2 class="w-title"><span class="pulse" aria-hidden="true"></span>Needs you <span class="serif-num" data-needs style="font-size:15px;color:var(--ink)">2</span><button class="more" data-to="requests">All requests ${ic('caret-right')}</button></h2>${permRow}${qRow}`;
  const lt = !v ? largeTitle('Home', 'Wednesday 25 November', 2)
    : `<header class="lt"><div><p class="date">Wednesday 25 November</p><h1>Home</h1></div><div class="lt-r">${unknownReq ? `<button class="req-btn" data-to="requests" aria-label="Requests, unknown">${ic('tray')}</button>` : needsPill(1)}</div></header>`;
  return `${chrome()}<div class="scroll">
    ${lt}
    ${v ? banner(v) : ''}
    <button class="gstrip" data-to="progress" aria-label="Planner, 1,240 of 1,500 XP. 12-day streak. Open Progress.">
      <span class="g-gauge">${powerReserve(40, reserve, true)}</span>
      <span class="g-lvl"><b>Planner</b><span class="g-xp"><span data-xp>1,240</span> / 1,500 XP</span></span>
      <span class="g-plus" aria-hidden="true">+40</span>
      <span class="datewheel" aria-hidden="true"><span class="dw-win"><span class="dw-col"><i>11</i><b>12</b><i>13</i></span></span><span class="dw-l">day<br>streak</span></span>
    </button>
    <div class="hv-toast" role="status" hidden>
      <span class="hv-medal">${ic('plant', 1)}</span>
      <span class="hv-t"><b>Harvested</b><span>claude-Resal-Dev01's week reset with 4% unused</span></span>
      <span class="hv-xp serif-num">+40</span>
    </div>

    <div class="board${stale ? ' is-stale' : ''}">
      <article class="card span2" style="--i:0" aria-label="Needs you">
        <button class="grip" aria-label="Move Needs you">${ic('dots-six-vertical')}</button>
        ${needs}
      </article>

      <article class="card span2" style="--i:1" aria-label="Use next">
        <button class="grip" aria-label="Move Use next">${ic('dots-six-vertical')}</button>
        <h2 class="w-title">Use next <span class="more">${stale ? `<span class="chip chip-amber chip-xs">${ic('clock-counter-clockwise')}${age} old</span>` : `<span class="age">${ic('clock-counter-clockwise')}read 8 m ago</span>`}</span></h2>
        <button class="un" data-to="account" aria-label="claude-Resal-Dev01: 64% of the week left, erased Saturday 18:58. Open the account.">
          <div class="dialwrap">${dial(116, .64, { w: 9, label: '64% of the week left', cls: stale ? 'stale' : '' })}<div class="c"><b>${stale ? '<small>≈</small>' : ''}64%</b><span>week left</span></div></div>
          <div><p class="acct">${tool('claude')}claude-Resal-Dev01</p>
            <p class="why-strong">64% erased Sat 18:58</p>
            <p class="why">${stale ? `As of ${age} ago. It may have changed since.` : 'The soonest loss. teksa keeps 75% to Tuesday.'}</p></div>
        </button>
        <div class="harvest"><div><b>${ic('plant')}Harvest window</b><span>Before Sat 18:58 · +40 XP</span></div>
          <button class="chip chip-tap" data-remind>${ic('bell-simple')}Remind me</button></div>
      </article>

      <article class="card" style="--i:2" aria-label="Running now">
        <button class="grip" aria-label="Move Running now">${ic('dots-six-vertical')}</button>
        <h2 class="w-title">Running</h2>
        <div class="sm-big serif-num">${stale ? '≈' : ''}7 <small>sessions</small></div>
        <div class="kv">DEVELOPMENTPC 5<br>LAPTOP 2<br><span class="dim">MINI asleep, 2 d</span></div>
      </article>

      <article class="card" style="--i:3" aria-label="Next reset">
        <button class="grip" aria-label="Move Next reset">${ic('dots-six-vertical')}</button>
        <h2 class="w-title">Next reset</h2>
        <div class="sm-big serif-num">1 h 48 <small>m</small></div>
        <div class="kv">sabushanab-resal<br>5-hour window<br><span class="dim">93% left at 23:10</span></div>
      </article>

      <article class="card span2" style="--i:4" aria-label="Today">
        <button class="grip" aria-label="Move Today">${ic('dots-six-vertical')}</button>
        <div class="today">
          <div><h2 class="w-title">Today</h2><div class="sm-big serif-num">${stale ? '≈' : ''}2.23 B <small>tokens</small></div>
            <div class="kv">≈ $737 at list price</div></div>
          <div><div class="spark" aria-label="Tokens per day, last 30 days">${days30.map((d, i) => `<span class="${i === 29 ? 'now' : ''}" style="--h:${(d / max * 100).toFixed(1)}"></span>`).join('')}</div>
            <div class="cap" style="text-align:right;margin-top:4px">30 days</div></div>
        </div>
      </article>
    </div>
    <button class="btn btn-quiet btn-sm arrange" data-arrange>${ic('arrows-down-up')}Arrange widgets</button>
  </div>${tabbar('Home')}`;
}
/* XP as a watch power reserve: a 240° arc with ticks */
function powerReserve(size, frac, compact) {
  const w = size, h = compact ? size * .9 : size * .7, cx = w / 2, cy = compact ? size / 2 + 2 : size / 2, r = size / 2 - (compact ? 6 : 12);
  const C = 2 * Math.PI * r, span = 240 / 360 * C;
  return `<svg class="dial" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" data-g="${cx},${cy},${r}" role="img" aria-label="${Math.round(frac * 100)}% of the way to the next level">
    ${tickRing(cx, cy, r + (compact ? 5 : 9), compact ? 13 : 25, compact ? 2 : 3, compact ? 3 : 4, compact ? 4 : 7, -120, 240)}
    <circle class="track" cx="${cx}" cy="${cy}" r="${r}" stroke-width="${compact ? 4 : 7}" stroke-linecap="round" transform="rotate(150 ${cx} ${cy})" style="stroke-dasharray:${span.toFixed(2)} ${C.toFixed(2)}"/>
    <circle class="arc lume" cx="${cx}" cy="${cy}" r="${r}" stroke-width="${compact ? 4 : 7}" transform="rotate(150 ${cx} ${cy})"
      style="--c:${C.toFixed(2)};--v:${(C - span * frac).toFixed(2)}"/></svg>`;
}

/* ---------- S9 Permission ---------- */
function permission() {
  return `${chrome()}<div class="scroll" style="padding-bottom:190px">
    ${navbar('Home', 'Permission')}
    <div class="perm">
      <div class="who">${tool('claude')}Claude Code · sabushanab-resal</div>
      <h2>Wants to run a command</h2>
      <div class="cmd"><code>npm run test -- --watch=false</code><p>Its description: “Run the unit tests”</p></div>
      <section class="ctx card" aria-label="What led to this request">
        <div class="ctx-s"><span class="k">Session</span><b>Account limit re-login notifications</b><span class="cap">started 3 h 11 m ago · edited 3 files this turn</span></div>
        <blockquote class="last"><span class="k">Claude, just before asking</span>
          <p>“The re-login banner and its notification are in. I'll run the unit tests to check nothing else broke.”</p></blockquote>
      </section>
      <ul class="where">
        <li>${ic('folder-simple')}<div><div class="k">In the folder</div><div class="v mono">D:\\<wbr>Projects\\<wbr>abushanab-net\\<wbr>HarnessManager-rc-chat</div></div></li>
        <li>${ic('desktop-tower')}<div><div class="k">On the computer</div><div class="v">DEVELOPMENTPC <span class="cap">· this network, direct</span></div></div></li>
      </ul>
      <p class="fine" style="margin:4px 2px 0">${ic('info')} If someone answers at the computer first, this request closes here.</p>
    </div>
  </div>
  <div class="actions">
    <div class="timer">
      <span style="width:40px;height:40px">${countdown(40, 252, 300, 'p2')}</span>
      <div><div class="left"><b class="serif-num" data-left="p2" data-start="252">4:12</b> to answer</div>
        <p data-pnote>Open until 21:26, then it stays in the terminal</p></div>
    </div>
    <div class="row"><button class="btn btn-secondary" data-deny>Deny</button>
      <button class="btn btn-primary" data-approve>${ic('scan')}Approve once</button></div>
    <p class="cap note">Approve once asks for Face ID and allows this command only.</p>
  </div>
  <section class="receipt" aria-live="polite">
    <div class="seal">${dial(96, 1, { w: 4, cls: 'lume', ticks: 60 })}${ic('check', 0)}</div>
    <h2 data-rt>Approved once</h2>
    <p data-rp>DEVELOPMENTPC confirmed it at 21:22:48. The tests are running.</p>
    <div class="card rows">
      <div><span>Command</span><span class="mono" style="font-size:12.5px">npm run test -- --watch=false</span></div>
      <div><span>Answered by</span><span>This phone, Face ID</span></div>
      <div><span>Delivered</span><span>Direct · 140 ms</span></div>
      <div><span>Session</span><span>Account limit re-login…</span></div>
    </div>
    <button class="btn btn-primary done" data-reset data-to="home">Back to Home</button>
  </section>`;
}

/* ---------- S15 Account detail: claude-Resal-Dev01 ---------- */
/* outer bezel = the week as time (grey = gone), inner ring = room left. Markers carry their own labels: no legend */
function resetClock() {
  const S = 350, c = 175, rT = 124, rR = 98;
  const elapsed = (168 - 69.6) / 168;                    // week window: 98.4 h gone of 168 h
  const Ct = 2 * Math.PI * rT;
  const start = new Date(2026, 10, 21, 18, 58);          // window start: Sat 21 Nov 18:58
  const mids = [];                                       // midnights inside the window
  for (let d = 22; d <= 28; d++) mids.push((new Date(2026, 10, d) - start) / 36e5 / 168);
  const now = elapsed * 360, fc = (168 - 4) / 168 * 360; // ≈ could run out Sat 15:00
  const [hx1, hy1] = pt(c, c, rT - 12, now), [hx2, hy2] = pt(c, c, rT + 13, now);
  const [fx, fy] = pt(c, c, rT + 11, fc);
  const lab = (deg, r, anchor, l1, l2, cls = '') => { const [x, y] = pt(c, c, r, deg);
    return `<text class="dl ${cls}" x="${x.toFixed(1)}" y="${(y - (l2 ? 3 : -4)).toFixed(1)}" text-anchor="${anchor}">${l1}</text>${l2 ? `<text class="dl2" x="${x.toFixed(1)}" y="${(y + 11).toFixed(1)}" text-anchor="${anchor}">${l2}</text>` : ''}`; };
  return `<svg class="dial" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="Week window: 98 hours gone, 70 to go, resets Saturday 18:58. 64% of the room left. It could run out around Saturday 15:00.">
    ${tickRing(c, c, rT + 9, 168, 2.5, 0)}
    ${mids.map(m => { const [x1, y1] = pt(c, c, rT + 12, m * 360), [x2, y2] = pt(c, c, rT + 4, m * 360); return `<line class="tick major" x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`; }).join('')}
    <circle class="track" cx="${c}" cy="${c}" r="${rT}" stroke-width="6"/>
    <circle class="arc" cx="${c}" cy="${c}" r="${rT}" stroke-width="6" transform="rotate(-90 ${c} ${c})" style="stroke:var(--ink-3);opacity:.55;--c:${Ct.toFixed(2)};--v:${(Ct * (1 - elapsed)).toFixed(2)}"/>
    ${ringArc(c, c, rR, 14, .64)}
    <line class="hand" x1="${hx1}" y1="${hy1}" x2="${hx2}" y2="${hy2}" style="stroke-width:2.5"/>
    <path d="M ${fx} ${fy} l -5 -8 h 10 z" transform="rotate(${fc} ${fx} ${fy})" style="fill:var(--amber)"/>
    <path d="M ${c} ${c - rT - 3} l -5.5 -10 h 11 z" style="fill:var(--lume)"/>
    ${lab(8, rT + 30, 'start', 'Reset', 'Sat 18:58', 'lume')}
    ${lab(-9, rT + 30, 'end', '≈ could run out', 'Sat 15:00', 'amber')}
    ${lab(now, rT + 26, 'end', 'Now', 'Wed 21:22')}
    ${lab(142, rT + 22, 'start', '98 h gone', '')}
    ${lab(312, rT + 22, 'end', '70 h to go', '')}
  </svg>`;
}
/* Revised for the full phase: the decision (room, reset, use next) fits the first 844 pt; forecast, 7 days and sessions sit in tabs below. */
function account() {
  const d7 = [0, 0, 0, 0, 0, 825051, 4164242], mx = Math.max(...d7), dn = ['Thu', 'Fri', 'Sat', 'Sun', 'Mon', 'Tue', 'Wed'];
  const fmt = v => v === 0 ? '0' : v >= 1e6 ? (v / 1e6).toFixed(2) + ' M' : Math.round(v / 1e3) + ' k';
  const win = (n, v, r) => `<div class="wrow"><b>${n}</b><div class="bar${v < .2 ? ' low' : ''}"><span style="--v:${v}"></span></div><span class="serif-num">${Math.round(v * 100)}%</span><span class="wr">${r}</span></div>`;
  return `${chrome()}<div class="scroll">
    ${navbar('Accounts', '', needsPill(2))}
    <header class="acc-head"><h1>claude-Resal-Dev01</h1>
      <div class="sub">${tool('claude')}Claude · Max 5x <span class="sep"></span><span class="age">${ic('clock-counter-clockwise')}read 8 m ago, live</span><span class="chip chip-xs">${ic('circle', 1)}2 running</span></div></header>

    <div class="rclock">${resetClock()}<div class="c"><b class="serif-num">64%</b><span>room left this week</span></div></div>

    <section class="decide card" aria-label="The decision">
      <div class="d-top"><span class="chip chip-xs chip-ink">${ic('push-pin', 1)}Use next</span><span class="cap">the soonest loss of your 9 accounts</span></div>
      <p class="d-big">64% erased Sat 18:58 if unused</p>
      <div class="harvest"><div><b>${ic('plant')}Harvest window</b><span>Use it first · +40 XP</span></div>
        <button class="chip chip-tap" data-remind>${ic('bell-simple')}Remind me Sat 09:00</button></div>
    </section>

    <section class="sec tight"><div class="card wins">
      ${win('5-hour', .52, '23:40')}${win('Week', .64, 'Sat 18:58')}${win('Fable', 1, 'with week')}
    </div></section>

    <section class="sec">
      <div class="tabs" role="tablist" aria-label="More about this account">
        <button role="tab" aria-selected="true" data-tab-to="fc">Forecast</button><button role="tab" aria-selected="false" data-tab-to="d7">7 days</button><button role="tab" aria-selected="false" data-tab-to="ss">Sessions</button></div>
      <div class="tabpanel" data-panel="fc"><div class="card forecast">${ic('trend-down')}
        <div><b>≈ Between running out Saturday afternoon and ≈ 30% left at reset</b>
          <p>All of this week's use is from the last two days, so the range is wide. At the last two days' pace it runs out around Sat 15:00; at the 7-day average about 30% is left.</p></div></div></div>
      <div class="tabpanel" data-panel="d7" hidden><div class="card week">
        <div class="tot"><b class="serif-num">4.99 M <small style="font:500 13px var(--sans);color:var(--ink-3)">tokens</small></b><span>16 sessions</span></div>
        <div class="bars7">${d7.map((v, i) => `<div class="b${i === 6 ? ' today' : ''}"><div class="col"><span class="${v ? '' : 'zero'}" style="--h:${(v / mx * 82).toFixed(1)}"></span>${v ? `<em style="--h:${(v / mx * 82).toFixed(1)}">${fmt(v)}</em>` : ''}</div><small>${dn[i]}</small></div>`).join('')}</div>
        <p class="cap" style="margin:12px 0 0">Thu to Mon: no sessions on this account. Cost per account: not split yet (—).</p></div></div>
      <div class="tabpanel" data-panel="ss" hidden><div class="card rows">
        <div class="srow"><div><b>Delegated task</b><span>subscription-entitlements · started 33 m ago</span></div><span class="st">${ic('circle', 1)}Working</span></div>
        <div class="srow"><div><b>Delegated task</b><span>subscription-entitlements · started 1 h 39 m ago</span></div><span class="st">${ic('circle', 1)}Working</span></div>
        <div class="srow"><div><b>Branch protection and default branch setup</b><span>subscription-entitlements · yesterday</span></div><span class="cap">Ended</span></div>
      </div></div>
    </section>
  </div>${tabbar('Accounts')}`;
}
/* ---------- S18 Progress ---------- */
/* Badges: rule, and the real event behind each (invented, consistent with data.json) */
const BADGES = [
  { id: 'first', icon: 'plant', name: 'First harvest', rule: "Use an account's room before its reset", earned: 'Mon 9 Nov', why: "claude-main's week reset with 6% unused. The first reset you used up before it erased the room." },
  { id: 'two', icon: 'desktop-tower', name: 'Two computers', rule: 'Pair a second computer', earned: 'Thu 12 Nov', why: 'LAPTOP was paired and granted this phone. Sessions there now show up here.' },
  { id: 'seven', icon: 'calendar-check', name: 'Seven days', rule: 'A 7-day streak', earned: 'Wed 18 Nov', why: 'Seven days in a row with recorded work. A sync gap counts as unknown and never breaks a streak.' },
  { id: 'even', icon: 'scales', name: 'Even load', rule: 'No Claude account runs out for a whole week', earned: 'Sun 22 Nov', why: 'From 16 to 22 Nov every Claude account reached its reset with room left.' },
  { id: 'fortnight', icon: 'calendar-plus', name: 'Fortnight', rule: 'A 14-day streak', prog: [12, 14], unit: 'days', why: 'Two more days with recorded work, on any computer.' },
  { id: 'sweep', icon: 'broom', name: 'Clean sweep', rule: 'Harvest every reset in one week', prog: [3, 4], unit: 'resets', why: 'One reset left this week: claude-Resal-Dev01, Sat 18:58, with 64% of its week still unused.' },
];
let medalId = 0;
/* engraved medallion: knurled rim, bead ring, enamel field, embossed icon. Locked: dashed rim with a lume progress arc */
function medallion(S, b) {
  const c = S / 2, R = c - 1, id = 'mg' + (medalId++), earned = !!b.earned, big = S >= 120;
  const Cp = 2 * Math.PI * (R - 1.5), frac = b.prog ? b.prog[0] / b.prog[1] : 1;
  const name = (b.name + ' · Harness Manager').toUpperCase();
  return `<div class="medal${earned ? '' : ' locked'}" style="width:${S}px;height:${S}px"><svg class="dial" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" aria-hidden="true">
    <defs><radialGradient id="${id}" cx="40%" cy="32%" r="75%"><stop offset="0" style="stop-color:var(--paper)"/><stop offset="1" style="stop-color:var(--sunk)"/></radialGradient>
      ${big ? `<path id="${id}p" d="M ${c} ${c} m -${R * .79} 0 a ${R * .79} ${R * .79} 0 1 1 ${R * 1.58} 0 a ${R * .79} ${R * .79} 0 1 1 -${R * 1.58} 0"/>` : ''}</defs>
    <circle cx="${c}" cy="${c}" r="${R}" fill="${earned ? `url(#${id})` : 'none'}" class="m-rim"/>
    ${earned ? tickRing(c, c, R, big ? 120 : 64, big ? 4 : 2.6, 0).replaceAll('class="tick"', 'class="tick knurl"') : ''}
    ${!earned ? `<circle cx="${c}" cy="${c}" r="${R - 1.5}" fill="none" class="m-prog" stroke-width="${big ? 4 : 3}" transform="rotate(-90 ${c} ${c})" style="stroke-dasharray:${(Cp * frac).toFixed(2)} ${Cp.toFixed(2)}"/>` : ''}
    <circle cx="${c}" cy="${c}" r="${R * (big ? .9 : .86)}" fill="none" class="m-line"/>
    ${big ? `<text class="m-txt"><textPath href="#${id}p" startOffset="50%" text-anchor="middle">${name}</textPath></text>`
          : `<circle cx="${c}" cy="${c}" r="${R * .74}" fill="none" class="m-bead"/>`}
    <circle cx="${c}" cy="${c}" r="${R * .6}" fill="url(#${id})" class="m-field"/>
    ${earned ? `<circle cx="${c}" cy="${c - R * .86}" r="${big ? 3.5 : 2.2}" class="m-jewel"/>` : ''}
  </svg>${ic(b.icon, earned)}</div>`;
}
function progress() {
  // 14 days: Thu 12 Nov .. Wed 25 Nov. Streak = last 12 days; Fri 20 is a sync gap (unknown, kept)
  const days = ['T', 'F', 'S', 'S', 'M', 'T', 'W', 'T', 'F', 'S', 'S', 'M', 'T', 'W'];
  const state = i => i === 1 ? 'off' : i === 8 ? 'gap' : i === 13 ? 'today' : '';
  const harvested = [1, 1, 1, 0];
  return `${chrome()}<div class="scroll">
    ${navbar('You', '', needsPill(2))}
    <header class="lt" style="padding-top:0"><h1>Progress</h1></header>

    <section class="card lvl-hero">
      <div class="lvl-top" data-levelup title="Preview the level-up">
        <div class="reserve">${powerReserve(150, 1240 / 1500)}<div class="c"><b class="serif-num">1,240</b><span>of 1,500 XP</span></div></div>
        <div><h2>Planner</h2><p>Level 3 of 4. 260 XP to Strategist.</p></div>
      </div>
      <div class="next-step"><span class="ns-k">${ic('plant')}Next</span><p>Harvest claude-Resal-Dev01 before Sat 18:58: <b>+40 XP</b></p><button class="chip chip-tap" data-to="account">Open</button></div>
      <div class="ladder" style="--p:.667"><span class="fill"></span>
        <div class="done"><i>${ic('check')}</i>Connected</div><div class="done"><i>${ic('check')}</i>Organized</div>
        <div class="cur"><i></i>Planner</div><div><i></i>Strategist</div></div>
    </section>

    <section class="sec tight" style="--i:1"><div class="card streak-card">
      <div class="sk-row">
        <span class="datewheel big" aria-hidden="true"><span class="dw-win"><span class="dw-col"><i>11</i><b>12</b><i>13</i></span></span></span>
        <div class="sk-t"><b>12-day streak</b><span>best 21 · dashed: a sync gap, never a break</span></div></div>
      <div class="days">${days.map((d, i) => `<div class="${state(i)}"><i></i>${d}</div>`).join('')}</div>
          </div></section>

    <section class="sec tight" style="--i:2"><div class="card wk">
      <div><div class="cap">Resets harvested this week</div><b class="serif-num">3 <small>of 4</small></b>
        <div class="mini">${harvested.map(h => h ? dial(22, 1, { w: 3, ticks: 0, cls: 'lume', label: 'harvested' }) : dial(22, 0, { w: 3, ticks: 0, label: 'still open' })).join('')}</div></div>
      <div><div class="cap">Room left unused</div><b class="serif-num">≈ 18%</b><div class="bar" style="margin-top:10px"><span style="--v:.18;background:var(--ink-3)"></span></div></div>
    </div></section>

    <section class="sec" style="--i:3">
      <div class="tabs" role="tablist" aria-label="More progress">
        <button role="tab" aria-selected="true" data-tab-to="bd">Badges <small>7 of 18</small></button><button role="tab" aria-selected="false" data-tab-to="xp">XP log</button><button role="tab" aria-selected="false" data-tab-to="rc">Recap</button></div>
      <div class="tabpanel" data-panel="bd"><div class="badges">
        ${BADGES.map(b => `<button class="card badge${b.earned ? '' : ' locked'}" data-badge="${b.id}">${medallion(76, b)}<b>${b.name}</b>
          <span>${b.earned ? b.rule : `${b.prog[0]} of ${b.prog[1]} ${b.unit}`}</span></button>`).join('')}
      </div></div>
      <div class="tabpanel" data-panel="xp" hidden><div class="card xplog">
        <div><b>+40</b><p>service-account-resal's week reset with 4% unused<span>Wed 08:00 · harvested</span></p></div>
        <div><b>+40</b><p>codex-main's week reset with ≈ 3% unused<span>Tue 18:00 · harvested</span></p></div>
        <div><b>+40</b><p>samykabu-teksa's week reset with ≈ 9% unused<span>Tue 10:00 · harvested</span></p></div>
        <div><b class="nil">0</b><p>agy-main's week reset unused<span>Today 21:16 · missed, no penalty</span></p></div>
      </div></div>
      <div class="tabpanel" data-panel="rc" hidden><article class="card recap">
        <svg class="rings dial" viewBox="0 0 240 240" aria-hidden="true">${tickRing(120, 120, 118, 60, 4, 5, 9)}<circle cx="120" cy="120" r="88" fill="none" stroke="var(--rule)"/><circle cx="120" cy="120" r="56" fill="none" stroke="var(--rule)"/></svg>
        <div class="wk-l">Week 47 · 16 to 22 November</div>
        <h4>4 of 5 resets harvested</h4>
        <dl><div><dt>Room left unused</dt><dd class="serif-num">≈ 14%</dd></div><div><dt>XP earned</dt><dd class="serif-num">+360</dd></div>
          <div><dt>Streak at week's end</dt><dd class="serif-num">9 days</dd></div><div><dt>Busiest project</dt><dd>Bunyan</dd></div></dl>
        <div class="foot"><span>Harness Manager</span><button class="chip chip-tap">${ic('export')}Share as image</button></div>
      </article></div>
    </section>
    <p class="fine">Points come only from room and time saved, never from tokens, money or how fast you approve. You can turn the game layer off in Settings.</p>
  </div>${tabbar('You')}
  <div class="scrim" hidden data-close></div>
  <section class="sheet" role="dialog" aria-modal="true" aria-label="Badge" hidden></section>
  ${levelUp()}`;
}
function badgeSheet(b) {
  const status = b.earned ? `<p class="bs-when">${ic('seal-check', 1)}Earned ${b.earned}</p>`
    : `<div class="bs-prog"><div class="bar lume"><span style="--v:${(b.prog[0] / b.prog[1]).toFixed(3)}"></span></div><p>${b.prog[0]} of ${b.prog[1]} ${b.unit}</p></div>`;
  return `<span class="grabber" aria-hidden="true"></span>
    <button class="bs-x" data-close aria-label="Close">${ic('x')}</button>
    <div class="bs-medal">${medallion(156, b)}</div>
    <h3>${b.name}</h3><p class="bs-rule">${b.rule}</p>${status}
    <div class="bs-why"><span class="k">${b.earned ? 'What counted' : 'What is left'}</span><p>${b.why}</p></div>`;
}
/* one restrained level-up: the reserve winds to full, a single sweep of lume ticks, the new name sets in */
function levelUp() {
  const S = 236, c = S / 2, cy = c + 6, r = 92, C = 2 * Math.PI * r, span = 240 / 360 * C;
  let ticks = '';
  for (let i = 0; i < 25; i++) { const d = -120 + 240 * i / 24, [x1, y1] = pt(c, cy, r + 18, d), [x2, y2] = pt(c, cy, r + (i % 6 === 0 ? 8 : 12), d);
    ticks += `<line class="lt" style="--d:${i * 18}ms" x1="${x1.toFixed(1)}" y1="${y1.toFixed(1)}" x2="${x2.toFixed(1)}" y2="${y2.toFixed(1)}"/>`; }
  return `<section class="levelup" tabindex="-1" role="dialog" aria-label="Level up: Strategist" hidden>
    <div class="lu-dial"><svg class="dial" width="${S}" height="${S * .86}" viewBox="0 0 ${S} ${S * .86}" aria-hidden="true">${ticks}
      <circle class="track" cx="${c}" cy="${cy}" r="${r}" stroke-width="10" stroke-linecap="round" transform="rotate(150 ${c} ${cy})" style="stroke-dasharray:${span.toFixed(2)} ${C.toFixed(2)}"/>
      <circle class="lu-arc" cx="${c}" cy="${cy}" r="${r}" stroke-width="10" transform="rotate(150 ${c} ${cy})" style="--c:${C.toFixed(2)};--v0:${(C - span * 1240 / 1500).toFixed(2)};--v:${(C - span).toFixed(2)}"/></svg>
      <div class="lu-c"><span>Level</span><b class="serif-num">4</b></div></div>
    <p class="lu-k">Level up</p>
    <h2>Strategist</h2>
    <p class="lu-p">1,500 XP, every point from room or time you saved. Level 4 of 4: the badges keep going from here.</p>
    <button class="btn btn-primary" data-close>Continue</button>
  </section>`;
}

const SCREENS = { home, permission, account, progress };

/* ---------- earned moments ---------- */
function countUp(el, from, to, ms) {
  if (RM()) { el.textContent = to.toLocaleString('en'); return; }
  const t0 = performance.now();
  const step = t => { const k = Math.min(1, (t - t0) / ms), e = 1 - Math.pow(1 - k, 4);
    el.textContent = Math.round(from + (to - from) * e).toLocaleString('en'); if (k < 1) requestAnimationFrame(step); };
  requestAnimationFrame(step);
}
/* harvest: the reserve winds up, a lume tick lands at the new mark, +40 rises and settles beside the XP */
function playHarvest(ph) {
  const strip = ph.querySelector('.gstrip'); if (!strip) return;
  ph.querySelector('.hv-toast').hidden = true; strip.classList.remove('harvested');
  strip.querySelector('.landed')?.remove();
  const svg = strip.querySelector('svg'), arc = svg.querySelector('.arc'), [cx, cy, r] = svg.dataset.g.split(',').map(Number);
  const C = parseFloat(arc.style.getPropertyValue('--c')), span = C * 240 / 360;
  arc.style.setProperty('--v', (C - span * 1240 / 1500).toFixed(2)); void arc.getBoundingClientRect();
  requestAnimationFrame(() => {
    arc.style.setProperty('--v', (C - span * 1280 / 1500).toFixed(2));
    const d = -120 + 240 * 1280 / 1500, [x1, y1] = pt(cx, cy, r + 1, d), [x2, y2] = pt(cx, cy, r + 9, d);
    svg.insertAdjacentHTML('beforeend', `<line class="landed" x1="${x1.toFixed(2)}" y1="${y1.toFixed(2)}" x2="${x2.toFixed(2)}" y2="${y2.toFixed(2)}" style="transform-origin:${x1.toFixed(2)}px ${y1.toFixed(2)}px"/>`);
    strip.classList.add('harvested');
    strip.setAttribute('aria-label', 'Planner, 1,280 of 1,500 XP after a harvest. 12-day streak. Open Progress.');
    countUp(strip.querySelector('[data-xp]'), 1240, 1280, 600);
    const toast = ph.querySelector('.hv-toast'); toast.hidden = false; toast.classList.remove('in'); void toast.offsetWidth; toast.classList.add('in');
  });
}
function playLevelUp(ph) {
  const lu = ph.querySelector('.levelup'); if (!lu) return;
  lu.hidden = false; lu.classList.remove('play'); void lu.offsetWidth; lu.classList.add('play');
  lu.focus({ preventScroll: true });
}
function openBadge(ph, id) {
  const b = BADGES.find(x => x.id === id), sh = ph.querySelector('.sheet'), sc = ph.querySelector('.scrim');
  sh.innerHTML = badgeSheet(b); sh.hidden = false; sc.hidden = false;
  sh.classList.remove('in'); sc.classList.remove('in'); void sh.offsetWidth; sh.classList.add('in'); sc.classList.add('in');
}
function closeOverlays(ph) { ph.querySelectorAll('.sheet, .scrim, .levelup').forEach(el => { el.hidden = true; el.classList.remove('in', 'play'); }); }

/* ---------- weekly recap as a shareable 1080 x 1350 image ---------- */
function recapImage() {
  const S = 560, c = S / 2, resets = [[.13, 1], [.36, 1], [.52, 1], [.71, 0], [.9, 1]]; // position in the week, harvested?
  const marks = resets.map(([f, h]) => { const [x, y] = pt(c, c, 238, f * 360);
    return `<circle cx="${x.toFixed(1)}" cy="${y.toFixed(1)}" r="15" ${h ? 'style="fill:var(--lume)"' : 'style="fill:var(--paper);stroke:var(--ink-3);stroke-width:2.5;stroke-dasharray:5 5"'}/>`; }).join('');
  return `<div class="ri-head"><span>Harness Manager</span><span>Week 47</span></div>
    <div class="ri-dial"><svg class="dial" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" aria-hidden="true">
      ${tickRing(c, c, 276, 168, 8, 24, 20)}<circle cx="${c}" cy="${c}" r="238" fill="none" style="stroke:var(--rule);stroke-width:3"/>${marks}
      <circle cx="${c}" cy="${c}" r="196" fill="none" style="stroke:var(--rule)"/></svg>
      <div class="ri-c"><b class="serif-num">4 of 5</b><span>resets harvested</span></div></div>
    <dl class="ri-stats"><div><dt>Room left unused</dt><dd class="serif-num">≈ 14%</dd></div><div><dt>XP earned</dt><dd class="serif-num">+360</dd></div>
      <div><dt>Streak at week's end</dt><dd class="serif-num">9 days</dd></div><div><dt>Busiest project</dt><dd>Bunyan</dd></div></dl>
    <div class="ri-foot"><span>16 to 22 November 2026</span><span>Points for room saved, never for tokens spent</span></div>`;
}

/* ---------- mount, motion, behaviour ---------- */
const STATES = {
  harvest: ph => playHarvest(ph),
  levelup: ph => playLevelUp(ph),
  badge: (ph, id) => openBadge(ph, id),
};
function runState(ph) {
  if (!ph.dataset.state) return;
  const [name, arg] = ph.dataset.state.split(':'); STATES[name](ph, arg);
}
/* ---------- pages: a shared top bar, light/dark pairs, a full-scroll switch ---------- */
const PAGES = [['index.html', 'System'], ['onboarding.html', 'Onboarding'], ['requests.html', 'Home and requests'], ['sessions.html', 'Sessions'],
  ['accounts.html', 'Accounts'], ['you.html', 'You'], ['states.html', 'States'], ['platforms.html', 'Android and desktop'], ['widgets.html', 'Widgets'], ['prototype.html', 'Prototype']];
function topbar() {
  const el = document.querySelector('[data-topbar]'); if (!el) return;
  const here = decodeURIComponent(location.pathname.split('/').pop()) || 'index.html';
  el.className = 'topbar';
  el.innerHTML = `<a class="mark" href="index.html">Calibre <span>· Designer A</span></a>
    <nav aria-label="Pages">${PAGES.map(([h, t]) => `<a href="${h}" ${h === here ? 'aria-current="page"' : ''}>${t}</a>`).join('')}</nav>
    <span class="spacer"></span>
    ${document.querySelector('[data-pair]') ? '<button class="quiet-btn" data-full aria-pressed="false" title="Show every screen at its full scroll length">Full scroll</button>' : ''}
    <div class="seg" role="group" aria-label="Page theme"><button data-page-theme="light" aria-pressed="true">Light</button><button data-page-theme="dark" aria-pressed="false">Dark</button></div>
    <button class="quiet-btn" data-rm aria-pressed="false">Reduce motion</button>
    <button class="quiet-btn" data-replay>Replay</button>`;
}
/* <section data-pair="key" data-v="variant" data-os="android"> + its notes  →  notes, a light phone, a dark phone */
function pairs() {
  document.querySelectorAll('[data-pair]').forEach(p => {
    if (p.dataset.built) return; p.dataset.built = 1;
    p.classList.add('screen-set');
    const and = p.dataset.os === 'android';
    const frame = th => `<div><p class="frame-cap">${and ? 'Android · ' : ''}${th === 'light' ? (and ? 'light' : 'Light') : (and ? 'dark' : 'Dark')}${p.dataset.cap ? ' · ' + p.dataset.cap : ''}</p>
      <div class="device${and ? ' android' : ''}"><div class="phone${and ? ' android' : ''}" data-theme="${th}" data-screen="${p.dataset.pair}"
        ${p.dataset.v ? `data-v="${p.dataset.v}"` : ''} ${p.dataset.state ? `data-state="${p.dataset.state}"` : ''} ${p.hasAttribute('data-fixed') ? 'data-fixed' : ''}></div></div></div>`;
    const ios = p.dataset.ios ? `<div><p class="frame-cap">iOS · light</p><div class="device"><div class="phone" data-theme="light" data-screen="${p.dataset.ios}" ${p.hasAttribute('data-fixed') ? 'data-fixed' : ''}></div></div></div>` : '';
    if (ios || and) p.classList.add('cmp');
    p.insertAdjacentHTML('beforeend', ios + frame('light') + frame('dark'));
  });
}
function render(ph) {
  let html = SCREENS[ph.dataset.screen](ph.dataset.v || '', ph);
  if (ph.hasAttribute('data-static')) html = html.replace(/data-left=|data-cd=/g, 'data-off=');  // static artboards do not tick
  ph.innerHTML = html;
  if (ph.dataset.xq1) { delete ph.dataset.xq1; expire(ph, 'q1'); }   // an expired question stays expired after navigating
}
function mount() {
  medalId = 0;
  pairs();
  document.querySelectorAll('.phone[data-screen]').forEach(ph => { delete ph.dataset.xq1; render(ph); ph.classList.remove('is-in'); });
  document.querySelectorAll('[data-recap]').forEach(el => { el.innerHTML = recapImage(); });
  document.querySelectorAll('[data-ds]').forEach(el => { el.classList.remove('is-in'); });
  document.dispatchEvent(new Event('calibre:mount'));
  requestAnimationFrame(() => requestAnimationFrame(() => document.querySelectorAll('.phone, [data-ds], .wg').forEach(p => p.classList.add('is-in'))));
  setTimeout(() => document.querySelectorAll('[data-state]').forEach(runState), 700);
  startClock();
}

let timer, T0 = Date.now();
function startClock() {
  clearInterval(timer); T0 = Date.now();
  const tick = () => {
    const gone = Math.floor((Date.now() - T0) / 1000);
    document.querySelectorAll('[data-left]').forEach(el => {
      const left = Math.max(0, +el.dataset.start - gone);
      el.textContent = mmss(left);
      const ph = el.closest('.phone, .wg'); if (!ph) return;
      const arc = ph.querySelector(`[data-cd="${el.dataset.left}"]`);
      if (arc) { const C = parseFloat(arc.style.getPropertyValue('--c')); arc.style.setProperty('--v', (C * (1 - left / +arc.dataset.total)).toFixed(2)); }
      if (left === 0 && ph.classList.contains('phone') && !ph.dataset['x' + el.dataset.left]) expire(ph, el.dataset.left);
    });
  };
  tick(); timer = setInterval(tick, 1000);
}
/* a request that runs out: the question honestly goes back to the terminal (the permission has minutes, it is not shown expiring) */
function expire(ph, id) {
  ph.dataset['x' + id] = 1;
  if (id !== 'q1') return;
  ph.querySelectorAll('[data-req="q1"]').forEach(row => { row.classList.add('gone'); const m = row.querySelector('[data-qmeta]'); if (m) m.textContent = 'Went back to the terminal on LAPTOP'; });
  ph.querySelectorAll('[data-needs]').forEach(n => { n.textContent = Math.max(0, +n.textContent - 1); });
  ph.querySelectorAll('[data-soon]').forEach(s => { s.dataset.left = 'p1'; s.dataset.start = 252; });
  ph.querySelectorAll('[data-qscreen]').forEach(q => q.classList.add('expired'));
}

/* ---------- prototype navigation: push, back, tabs ---------- */
const HIST = new WeakMap();
function go(ph, to, back) {
  const h = HIST.get(ph) || [], tab = TABS.some(t => t[2] === to) && !back;
  if (back) { if (!h.length) return; to = h.pop(); }
  else if (to === ph.dataset.screen && !ph.dataset.v) return;
  else if (tab) h.length = 0;
  else h.push(ph.dataset.screen);
  HIST.set(ph, h);
  clearTimeout(ph._auto);
  ph.dataset.screen = to; delete ph.dataset.v;
  render(ph);
  ph.classList.remove('is-in', 'nav-push', 'nav-back', 'nav-tab'); void ph.offsetWidth;
  ph.classList.add(back ? 'nav-back' : tab ? 'nav-tab' : 'nav-push');
  requestAnimationFrame(() => requestAnimationFrame(() => ph.classList.add('is-in')));
  const auto = ph.querySelector('[data-auto]');                  // screens that move on by themselves (approval on the computer)
  if (auto) { const [next, ms] = auto.dataset.auto.split(':'); ph._auto = setTimeout(() => ph.dataset.screen === to && go(ph, next), +ms); }
  document.dispatchEvent(new CustomEvent('proto:nav', { detail: { ph, to } }));
}
function showReceipt(ph, title, text, ok = true) {
  const rc = ph.querySelector('.receipt'); if (!rc) return;
  rc.classList.toggle('denied', !ok);
  rc.querySelector('[data-rt]').textContent = title;
  rc.querySelector('[data-rp]').textContent = text;
  rc.querySelector('.seal i').className = ok ? 'ph ph-check' : 'ph ph-x';
  rc.querySelector('.seal .arc').classList.toggle('lume', ok);
  rc.classList.remove('show'); void rc.offsetWidth; rc.classList.add('show');
}

document.addEventListener('click', e => {
  const t = e.target.closest('button, a, [data-close], [data-levelup]'); if (!t) return;
  if (t.matches('a[href="#"]')) e.preventDefault();
  if (t.dataset.play) {                                       // page buttons that replay a moment in the named phones
    document.querySelectorAll(t.dataset.for).forEach(p => { closeOverlays(p); STATES[t.dataset.play](p, t.dataset.arg); });
    return;
  }
  const ph = t.closest('.phone'); if (!ph) return;
  const proto = ph.hasAttribute('data-proto');
  if (t.dataset.badge) { openBadge(ph, t.dataset.badge); return; }
  if (t.hasAttribute('data-levelup')) { playLevelUp(ph); return; }
  if (t.hasAttribute('data-close')) { closeOverlays(ph); return; }
  if (t.dataset.tabTo) {                                       // in-screen tabs (Account detail, Progress, Requests)
    const box = t.closest('.sec, .seg-box');
    box.querySelectorAll('[role=tab]').forEach(b => b.setAttribute('aria-selected', b === t));
    box.querySelectorAll('[data-panel]').forEach(p => { p.hidden = p.dataset.panel !== t.dataset.tabTo; });
    return;
  }
  if (t.dataset.pick !== undefined) {                          // single choice rows (question options, sizes)
    t.closest('[role=radiogroup]').querySelectorAll('[role=radio]').forEach(r => r.setAttribute('aria-checked', r === t));
    return;
  }
  if (t.hasAttribute('data-toggle')) { t.setAttribute('aria-checked', t.getAttribute('aria-checked') !== 'true'); return; }
  if (t.hasAttribute('data-approve') || t.hasAttribute('data-deny')) {
    const ok = t.hasAttribute('data-approve');
    const show = () => showReceipt(ph, ok ? 'Approved once' : 'Denied', ok ? 'DEVELOPMENTPC confirmed it at 21:22:48. The tests are running.'
      : 'DEVELOPMENTPC confirmed it at 21:22:48. Claude Code was told no and will carry on without running it.', ok);
    if (ok && !RM()) { t.classList.add('scanning'); t.lastChild.textContent = 'Face ID'; setTimeout(show, 600); } else show();
    return;
  }
  if (t.hasAttribute('data-done')) { showReceipt(ph, t.dataset.title, t.dataset.text); return; }
  if (t.hasAttribute('data-reset') && !proto) { render(ph); requestAnimationFrame(() => ph.classList.add('is-in')); return; }
  if (t.dataset.to || t.hasAttribute('data-back')) {
    if (proto) { e.preventDefault(); go(ph, t.dataset.to, t.hasAttribute('data-back')); return; }
    const target = t.dataset.to && document.querySelector(`[data-pair="${t.dataset.to}"]`);
    if (target && !t.closest('[data-pair="' + t.dataset.to + '"]')) target.scrollIntoView({ behavior: RM() ? 'auto' : 'smooth', block: 'start' });
    return;
  }
  if (t.hasAttribute('data-remind')) { t.innerHTML = `${ic('check')}Reminder set`; t.classList.add('chip-lume'); }
  if (t.hasAttribute('data-arrange')) {
    const b = ph.querySelector('.board'), on = b.classList.toggle('editing');
    t.innerHTML = on ? `${ic('check')}Done` : `${ic('arrows-down-up')}Arrange widgets`;
  }
});

/* drag to reorder with FLIP, so neighbours settle instead of jumping */
function flip(board, mutate) {
  const cards = [...board.children], before = new Map(cards.map(c => [c, c.getBoundingClientRect()]));
  mutate();
  cards.forEach(c => {
    if (c.classList.contains('dragging')) return;
    const a = before.get(c), b = c.getBoundingClientRect(), dx = a.left - b.left, dy = a.top - b.top;
    if (!dx && !dy) return;
    c.style.transition = 'none'; c.style.transform = `translate(${dx}px,${dy}px)`;
    requestAnimationFrame(() => { c.style.transition = RM() ? 'none' : 'transform var(--t-settle) var(--ease-spring)'; c.style.transform = ''; });
  });
  return before;
}
document.addEventListener('pointerdown', e => {
  const g = e.target.closest('.board.editing .grip'); if (!g) return;
  const card = g.closest('.card'), board = card.parentElement;
  e.preventDefault();  // listen on window: moving the card in the DOM drops pointer capture
  let ox = e.clientX, oy = e.clientY, last = null;
  card.classList.add('dragging');
  const move = ev => {
    card.style.transform = `translate(${ev.clientX - ox}px,${ev.clientY - oy}px) scale(1.02)`;
    const over = [...board.children].find(o => { if (o === card) return false; const r = o.getBoundingClientRect(); return ev.clientX > r.left && ev.clientX < r.right && ev.clientY > r.top && ev.clientY < r.bottom; });
    if (over === last) return;  // one swap per target, or the two cards trade places on every move
    last = over;
    if (!over) return;
    const r0 = card.getBoundingClientRect(), t0 = card.style.transform; card.style.transform = '';
    const home0 = card.getBoundingClientRect();
    flip(board, () => { const after = [...board.children].indexOf(over) > [...board.children].indexOf(card); board.insertBefore(card, after ? over.nextSibling : over); });
    const home1 = card.getBoundingClientRect();
    ox += home1.left - home0.left; oy += home1.top - home0.top; card.style.transform = t0; void r0;
  };
  const up = () => {
    removeEventListener('pointermove', move); removeEventListener('pointerup', up); removeEventListener('pointercancel', up);
    card.classList.remove('dragging'); card.style.transition = RM() ? 'none' : 'transform var(--t-settle) var(--ease-spring)'; card.style.transform = '';
  };
  addEventListener('pointermove', move); addEventListener('pointerup', up); addEventListener('pointercancel', up);
});
/* keyboard: arrow keys on a grip move the card */
document.addEventListener('keydown', e => {
  const g = e.target.closest('.board.editing .grip'); if (!g || !['ArrowUp', 'ArrowDown'].includes(e.key)) return;
  e.preventDefault(); const card = g.closest('.card'), board = card.parentElement;
  flip(board, () => e.key === 'ArrowUp' ? card.previousElementSibling && board.insertBefore(card, card.previousElementSibling) : card.nextElementSibling && board.insertBefore(card.nextElementSibling, card));
  g.focus();
});

/* page controls (the top bar is built at load, so these are delegated) */
document.addEventListener('click', e => {
  const b = e.target.closest('[data-page-theme], [data-rm], [data-replay], [data-motion-demo], [data-full]'); if (!b) return;
  if (b.dataset.pageTheme) {
    document.documentElement.dataset.theme = b.dataset.pageTheme;
    document.querySelectorAll('[data-page-theme]').forEach(x => x.setAttribute('aria-pressed', x === b));
  } else if (b.hasAttribute('data-rm')) { b.setAttribute('aria-pressed', document.body.classList.toggle('rm')); }
  else if (b.hasAttribute('data-full')) { b.setAttribute('aria-pressed', document.body.classList.toggle('full')); }
  else if (b.hasAttribute('data-replay')) mount();
  else { const l = document.querySelector('.motion-list'); l.classList.remove('run'); void l.offsetWidth; l.classList.add('run'); }
});
if (new URLSearchParams(location.search).has('full')) document.body.classList.add('full');   // ?full for screenshots

/* cover and design-system specimens */
function specimens() {
  const cover = document.getElementById('cover-dial');
  if (cover) { cover.innerHTML = coverDial(); requestAnimationFrame(() => requestAnimationFrame(() => cover.classList.add('is-in'))); }
  const set = (id, html) => { const el = document.getElementById(id); if (el) el.innerHTML = html; };
  set('ds-dial', `<div style="position:relative;width:116px;height:116px">${dial(116, .64, { w: 9, label: '64%' })}<div style="position:absolute;inset:0;display:grid;place-content:center;text-align:center"><b class="serif-num" style="font-size:30px">64%</b><span class="cap">week left</span></div></div>`);
  set('ds-dial-s', `${dial(56, .11, { w: 5, cls: 'amber', label: '11%' })}`);
  set('ds-cd', `<div style="display:grid;justify-items:center;gap:4px">${countdown(44, 252, 300, 'ds')}<b style="font:600 13px var(--sans)">4:12</b></div>`);
  set('ds-res', powerReserve(96, 1240 / 1500));
}
function coverDial() {
  const S = 520, c = 260;
  let s = `<svg class="dial" viewBox="0 0 ${S} ${S}" role="img" aria-label="The Calibre dial: minute ticks, a room-left arc and a now hand">`;
  s += tickRing(c, c, 250, 120, 6, 10, 16);
  s += `<circle cx="${c}" cy="${c}" r="214" fill="none" stroke="var(--rule)"/>`;
  s += ringArc(c, c, 190, 22, .64);
  s += tickRing(c, c, 160, 7, 0, 1, 10);
  const [x1, y1] = pt(c, c, 150, 211), [x, y] = pt(c, c, 240, 211);
  s += `<line class="hand" x1="${x1}" y1="${y1}" x2="${x}" y2="${y}" style="stroke-width:3"/>`;
  const [s1x, s1y] = pt(c, c, 150, 80), [sx, sy] = pt(c, c, 232, 80);
  s += `<line x1="${s1x}" y1="${s1y}" x2="${sx}" y2="${sy}" style="stroke:var(--signal);stroke-width:2;stroke-linecap:round"/><circle cx="${s1x}" cy="${s1y}" r="4" fill="var(--signal)"/>`;
  s += `<text x="${c}" y="${c + 22}" text-anchor="middle" style="font:400 84px var(--serif);fill:var(--ink);letter-spacing:-2px">64%</text>`;
  s += `<text x="${c}" y="${c + 56}" text-anchor="middle" style="font:500 15px var(--sans);fill:var(--ink-3)">claude-Resal-Dev01 · resets Sat 18:58</text>`;
  return s + '</svg>';
}

document.addEventListener('DOMContentLoaded', () => { topbar(); specimens(); mount(); });

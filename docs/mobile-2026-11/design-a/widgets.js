/* Calibre · widget gallery. Native surfaces drawn to their real sizes and limits:
   iOS WidgetKit (systemSmall 170, systemMedium 364×170, systemLarge 364×382 pt on a 393-pt iPhone), Lock Screen accessory families
   (circular 72, rectangular 172×76, inline one line), ActivityKit (Lock Screen up to 160 pt, Dynamic Island compact / minimal / expanded),
   Android Glance widgets (cells, resizable) and the ongoing notification. Countdowns are Text(timerInterval:) / Chronometer: the system
   ticks them, the widget itself never animates. */

const W = {};
const wdial = (S, v, o = {}) => dial(S, v, { w: o.w || 6, ticks: o.ticks ?? 40, cls: o.cls || '', label: o.label || '' });
const tmr = (id, s) => `<b class="tm" data-left="${id}" data-start="${s}">${mmss(s)}</b>`;

/* ---------- iOS Home Screen ---------- */
W.s1 = () => `<div class="w w-s s1"><div class="w-dial">${wdial(104, .64, { w: 7, ticks: 48 })}<b class="serif-num">64<small>%</small></b></div>
  <p class="w-n">${tool('claude')}Dev01</p><p class="w-k">erased Sat 18:58</p></div>`;
W.s2 = () => `<div class="w w-s s2"><p class="w-h"><span class="pulse static"></span>Needs you</p><b class="w-big serif-num">2</b>
  <p class="w-k">soonest closes in</p><p class="w-t big">${tmr('wq', 41)}</p></div>`;
W.m1 = () => `<div class="w w-m m1"><div class="m-dials">${[['Dev01', .64, ''], ['resal', .29, ''], ['main', .11, 'amber']].map(([n, v, c]) =>
    `<div><span class="w-dial">${wdial(74, v, { w: 5, ticks: 0, cls: c })}<b class="serif-num">${Math.round(v * 100)}</b></span><small>${n}</small></div>`).join('')}</div>
  <div class="m-side"><p class="w-h"><span class="pulse static"></span>2 need you</p><p class="w-t">${tmr('wq', 41)}<small>soonest</small></p>
    <p class="w-k">Use next<br><b>Dev01</b>, 64% erased Sat</p></div></div>`;
W.m2 = () => {
  // a week ruler from now (Wed 21:22) to Tue 21:22: each reset with room left is a dot sized by the loss.
  // Labels sit on two rows under the ruler with leader lines, so Fri to Sun never collide; 11.5 pt minimum, the harvest at 13.
  const R = 324, x = h => +(h / 144 * R).toFixed(1), line = 18;
  const marks = [[41.6, 'main', 11, 40, 'end'], [51.6, 'resal', 29, 62, 'start'], [69.6, 'Dev01', 64, 40, 'start', 1], [132.6, 'teksa', 75, 40, 'end']];
  return `<div class="w w-m m2"><p class="w-h">Resets with room left <span class="w-age">8 m</span></p>
  <div class="ruler2"><i class="r-line"></i><i class="r-now"></i>
    ${['Thu', 'Fri', 'Sat', 'Sun', 'Mon', 'Tue'].map((d, i) => `<span class="rd" style="left:${x(2.6 + 24 * i)}px">${d}</span>`).join('')}
    ${marks.map(([h, n, v, row, a, hv]) => `<i class="ld${hv ? ' hl' : ''}" style="left:${x(h)}px;top:${line}px;height:${row - line}px"></i>
      <i class="mk${hv ? ' hl' : ''}" style="left:${x(h)}px;top:${line}px;--s:${6 + v / 8}px"></i>
      <span class="lb${hv ? ' hl' : ''}" style="top:${row}px;${a === 'end' ? `right:${R - x(h) - 6}px;text-align:right` : `left:${x(h) - 6}px`}">${n} ${v}%</span>`).join('')}
  </div>
  <p class="w-k">${ic('plant')} Harvest Dev01 before Sat 18:58 · +40 XP</p></div>`;
};
W.l1 = () => `<div class="w w-l l1"><p class="w-h"><span class="pulse static"></span>Needs you · 2 <span class="w-age">open to answer</span></p>
  <div class="l-req"><span>${ic('terminal')}Permission · DEVELOPMENTPC</span>${tmr('wp', 252)}</div>
  <div class="l-req"><span>${ic('chat-circle-text')}Question · LAPTOP</span>${tmr('wq', 41)}</div>
  <div class="l-dials">${[['Dev01', .64, ''], ['resal', .29, ''], ['main', .11, 'amber'], ['codex', .93, '']].map(([n, v, c]) =>
    `<div><span class="w-dial">${wdial(62, v, { w: 4.5, ticks: 0, cls: c })}<b class="serif-num">${Math.round(v * 100)}</b></span><small>${n}</small></div>`).join('')}</div>
  <div class="l-next"><span class="chip chip-xs chip-ink">${ic('push-pin', 1)}Use next</span><b>Dev01</b><span>64% erased Sat 18:58</span></div>
  <p class="w-k l-hv">${ic('plant')} Harvest window: use it first, +40 XP</p>
  <div class="l-foot"><span>${ic('terminal-window')}DEV PC 5 · LAPTOP 2</span><span>${powerReserve(26, 1240 / 1500, true)}Planner · 12 days</span></div></div>`;
W.l2 = () => {
  const rows = [['claude-Resal-Dev01', .64, 'Sat 18:58', 'claude', 'next'], ['claude-main', .11, 'Fri 14:58', 'claude', 'low'], ['sabushanab-resal', .29, 'Sat 00:58', 'claude', ''],
    ['samykabu-teksa', .75, 'Tue 09:58', 'claude', ''], ['codex-main', .93, 'Tue 17:58', 'codex', ''], ['service-account-resal', 1, 'Wed 07:58', 'claude', '']];
  return `<div class="w w-l l2"><p class="w-h">Week left, soonest loss first <span class="w-age">8 m</span></p>
  ${rows.map(([n, v, r, t, f]) => `<div class="l2-r${f === 'next' ? ' nx' : ''}">${tool(t)}<b>${n}</b><span class="bar${f === 'low' ? ' low' : ''}"><span style="--v:${v}"></span></span><em class="serif-num">${Math.round(v * 100)}</em><small>${r}</small></div>`).join('')}
  <p class="w-k">${ic('push-pin', 1)} Use next: Dev01, the soonest large loss</p></div>`;
};

/* ---------- Lock Screen (vibrant: the system renders these as one tint over the wallpaper) ---------- */
const gauge = (S, v, inner) => `<span class="lk-g">${dial(S, v, { w: 5, ticks: 0 })}<span>${inner}</span></span>`;
W.c1 = () => `<div class="lk lk-c">${gauge(68, .64, '<b>64</b><small>Dev01</small>')}</div>`;
W.c2 = () => `<div class="lk lk-c">${gauge(68, 41 / 60, `<b>2</b><small class="tm" data-left="wq" data-start="41">0:41</small>`)}</div>`;
W.r1 = () => `<div class="lk lk-r"><p class="lk-h">${ic('timer', 1)}Next reset, 1 h 48 m</p><p class="lk-b">resal 5-hour · 93% left</p><span class="lk-bar"><i style="width:93%"></i></span></div>`;
W.r2 = () => `<div class="lk lk-r r2">${[['Dev01', .64], ['resal', .29], ['main', .11]].map(([n, v]) => `<span>${gauge(44, v, `<b>${Math.round(v * 100)}</b>`)}<small>${n}</small></span>`).join('')}</div>`;
W.i1 = () => `<div class="lk lk-i"><span class="lk-date">Wed 25</span>${ic('gauge', 1)} Dev01 64% · Sat 18:58</div>`;
W.i2 = () => `<div class="lk lk-i"><span class="lk-date">Wed 25</span>${ic('bell-ringing', 1)} 2 need you · <span class="tm" data-left="wq" data-start="41">0:41</span></div>`;

/* ---------- Live Activity, Lock Screen ---------- */
W.la1d = () => `<div class="la la1"><div class="la-top"><span class="la-ic">${appIcon(28)}</span><b>Account limit re-login…</b><span class="la-st"><span class="pulse static"></span>Waiting for you</span></div>
  <p class="la-m">Permission on DEVELOPMENTPC · ${tmr('wp', 252)} left</p><span class="la-bar"><i style="width:${252 / 3}%"></i></span>
  <p class="la-f"><span>HarnessManager-rc-chat · 3 h 11 m</span><span>Open to answer</span></p></div>`;
/* default: private on the Lock Screen. No title, project or computer: the same words as the push alert */
W.la1 = () => `<div class="la la1"><div class="la-top"><span class="la-ic">${appIcon(28)}</span><b>Harness Manager</b><span class="la-st"><span class="pulse static"></span>Needs you</span></div>
  <p class="la-m">A session is waiting for you · ${tmr('wp', 252)} left</p><span class="la-bar"><i style="width:${252 / 3}%"></i></span>
  <p class="la-f"><span>Followed session · 3 h 11 m</span><span>${ic('lock-simple')} Unlock for details</span></p></div>`;
W.la2 = () => `<div class="la la2"><span class="la-d">${countdown(78, 252, 300, 'wp2')}<b class="tm" data-left="wp2" data-start="252">4:12</b></span>
  <div><p class="la-k">A session needs you</p><b class="la-q">Permission · DEVELOPMENTPC</b><p class="la-m">Open until 21:26. Then it stays in the terminal.</p>
  <p class="la-f"><span>${ic('lock-simple')} details after unlock</span></p></div></div>`;

/* ---------- Dynamic Island ---------- */
W.dc1 = () => `<div class="di di-c"><span class="di-l"><span class="pulse static"></span></span><span class="di-cam"></span><span class="di-r">${tmr('wp', 252)}</span></div>`;
W.dc2 = () => `<div class="di di-c"><span class="di-l">${dial(22, .64, { w: 3, ticks: 0 })}</span><span class="di-cam"></span><span class="di-r"><b>Dev01</b></span></div>`;
W.dm1 = () => `<div class="di di-m"><span class="pulse static"></span><b>2</b></div>`;
W.dm2 = () => `<div class="di di-m">${countdown(26, 41, 60, 'wq2')}</div>`;
W.de1 = () => `<div class="di di-e"><div class="de-top"><span class="la-ic">${appIcon(30)}</span><span><b>Account limit re-login…</b><small>DEVELOPMENTPC</small></span>${tmr('wp', 252)}</div>
  <p class="de-m"><span class="pulse static"></span>Waiting for you · Permission</p><div class="de-acts"><span>Open to answer</span></div></div>`;
W.de2 = () => `<div class="di di-e de2"><span class="la-d">${countdown(64, 41, 60, 'wq3')}<b class="tm" data-left="wq3" data-start="41">0:41</b></span>
  <div><small>Question · LAPTOP</small><b>Which branch should I base this on?</b><p>develop · main · feature/speckit-29</p></div></div>`;

/* ---------- Android, Glance (resizable) ---------- */
W.a1s = () => `<div class="aw aw-21 a1">${gauge2(56, .64)}<span><b>Dev01</b><small>to Sat 18:58</small></span></div>`;
W.a1m = () => `<div class="aw aw-22 a1">${gauge2(96, .64)}<p class="w-n">${tool('claude')}Dev01</p><p class="w-k">64% erased Sat 18:58</p></div>`;
W.a1l = () => `<div class="aw aw-42 a1 wide">${gauge2(104, .64)}<div><p class="w-k">Use next · read 8 m ago</p><p class="aw-n">claude-Resal-Dev01</p><p class="w-k">64% erased Sat 18:58</p>
  <p class="aw-hv">${ic('plant')} Harvest before Sat · +40 XP</p></div></div>`;
W.a2s = () => `<div class="aw aw-22 a2"><p class="w-h"><span class="pulse static"></span>Needs you</p><b class="w-big serif-num">2</b><p class="w-t">${tmr('wq', 41)}<small>soonest</small></p></div>`;
W.a2m = () => `<div class="aw aw-42 a2 wide"><div><p class="w-h"><span class="pulse static"></span>Needs you</p><b class="w-big serif-num">2</b><p class="w-t">${tmr('wq', 41)}<small>soonest</small></p></div>
  <div class="m-dials">${[['Dev01', .64, ''], ['resal', .29, ''], ['main', .11, 'amber']].map(([n, v, c]) => `<div><span class="w-dial">${wdial(58, v, { w: 4.5, ticks: 0, cls: c })}<b class="serif-num">${Math.round(v * 100)}</b></span><small>${n}</small></div>`).join('')}</div></div>`;
W.a2l = () => `<div class="aw aw-44 a2">${W.l1().replace('class="w w-l l1"', 'class="l1 in-aw"')}</div>`;
function gauge2(S, v) { return `<span class="w-dial">${wdial(S, v, { w: S > 80 ? 7 : 5, ticks: S > 80 ? 40 : 0 })}<b class="serif-num">${Math.round(v * 100)}</b></span>`; }

/* ---------- Android ongoing notification ---------- */
W.n1 = () => `<div class="an"><div class="an-h"><span class="an-ic">${ic('gauge', 1)}</span>Harness Manager · <span class="tm" data-left="wp" data-start="252">4:12</span> ${ic('caret-up')}</div>
  <b class="an-t">HarnessManager-rc-chat is waiting for you</b><p class="an-b">Permission on DEVELOPMENTPC · open until 21:26</p>
  <div class="an-acts"><span>Open to answer</span><span>Stop following</span></div></div>`;
/* the public version a secure lock screen shows: VISIBILITY_PRIVATE with setPublicVersion(), no actions */
W.n1p = () => `<div class="an"><div class="an-h"><span class="an-ic">${ic('gauge', 1)}</span>Harness Manager · <span class="tm" data-left="wp" data-start="252">4:12</span></div>
  <b class="an-t">A session needs you</b><p class="an-b">Unlock to see which one and answer it</p></div>`;
W.n2 = () => `<div class="sb-strip"><span>21:22</span><span class="status-chip">${ic('hourglass-medium', 1)}<span class="tm" data-left="wp" data-start="252">4:12</span></span><span class="sb-r">${ic('wifi-high', 1)}${ic('battery-high', 1)}</span></div><div class="an n2"><div class="an-h"><span class="an-ic">${ic('gauge', 1)}</span>Harness Manager · Live update ${ic('caret-up')}</div>
  <b class="an-t">A session needs you</b><p class="an-b">Closes in <span class="tm" data-left="wp" data-start="252">4:12</span> · then it waits in the terminal</p>
  <div class="an-prog"><i style="width:${(48 / 300 * 100).toFixed(1)}%"></i><span class="an-tr" style="left:${(48 / 300 * 100).toFixed(1)}%">${ic('hourglass-medium', 1)}</span><em style="left:0">opened 21:21</em><em style="right:0">21:26</em></div>
  <div class="an-acts"><span>Open to answer</span></div></div>`;

/* ---------- gallery ---------- */
const FAM = [
  ['iOS · Home Screen, small', 'home', [['A1', 's1', 'The Use next dial: one account, its room, the time it is erased. Answers job one with no text to read.'],
    ['A2', 's2', 'Needs you first: the count and the soonest countdown (a system timer). Says nothing about what is asked.']]],
  ['iOS · Home Screen, medium', 'home', [['A1', 'm1', 'Three dials, sorted by soonest loss, beside the requests and the pick.'],
    ['A2', 'm2', 'A week ruler: every reset with room left is a mark sized by the loss. The harvest calendar at a glance.']]],
  ['iOS · Home Screen, large', 'home', [['A1', 'l1', 'The whole Home board: requests with their timers, four dials, running per computer, level and streak.'],
    ['A2', 'l2', 'An accounts ledger: six accounts as bars, soonest loss first, the pick marked.']]],
  ['iOS · Lock Screen, circular', 'lock', [['A1', 'c1', 'accessoryCircular gauge of the Use next account.'], ['A2', 'c2', 'Waiting requests, the ring is the soonest timer (ProgressView with a timer interval).']]],
  ['iOS · Lock Screen, rectangular', 'lock', [['A1', 'r1', 'The next reset as a sentence and a bar.'], ['A2', 'r2', 'Three gauges in a row: the room on the accounts you use most.']]],
  ['iOS · Lock Screen, inline', 'lock', [['A1', 'i1', 'The pick and its reset, beside the date.'], ['A2', 'i2', 'Waiting count and the soonest timer, nothing else.']]],
  ['iOS · Live Activity, Lock Screen', 'lock', [['A1', 'la1|Default · private,la1d|Lock Screen detail: on', 'Following one session (started by Follow): its state and the request timer as a bar. Private by default, in the push alert’s words; the title, computer and project appear only with Settings › Lock Screen detail on.'],
    ['A2', 'la2', 'A request countdown on its own: the Calibre dial drains, private wording, details after unlock.']]],
  ['iOS · Dynamic Island, compact', 'island', [['A1', 'dc1', 'A static vermilion dot and the request timer.'], ['A2', 'dc2', 'No request: the Use next dial and its name.']]],
  ['iOS · Dynamic Island, minimal', 'island', [['A1', 'dm1', 'Shared with another activity: the dot and the count.'], ['A2', 'dm2', 'The soonest timer as a draining ring.']]],
  ['iOS · Dynamic Island, expanded', 'island', [['A1', 'de1', 'The followed session, its state and the timer, one action: open to answer.'],
    ['A2', 'de2', 'The question itself with its options, for a phone that allows detail on the Lock Screen.']]],
  ['Android · Home widget, resizable (Glance)', 'android', [['A1', 'a1s,a1m,a1l', 'Use next: 2×1 dial, 2×2 dial and reset, 4×2 adds the harvest line.'],
    ['A2', 'a2s,a2m,a2l', 'Needs you: 2×2 count and timer, 4×2 adds three dials, 4×4 becomes the board.']]],
  ['Android · Ongoing notification', 'shade', [['A1', 'n1p|Locked · public version,n1|Unlocked · full notification', 'A classic ongoing notification with a chronometer countdown. VISIBILITY_PRIVATE: a secure lock screen shows only the public version; project, computer and actions appear after unlock.'],
    ['A2', 'n2', 'An Android 16 Live Update (ProgressStyle): the request window as a progress bar with a tracker, promoted to a status-bar chip.']]],
];
(function gallery() {
  const el = document.querySelector('[data-widgets]'); if (!el) return;
  el.innerHTML = FAM.map(([t, ctx, vars]) => `<section class="fam"><h3>${t}</h3>${vars.map(([id, keys, why]) => `<div class="var${ctx === 'android' || keys.includes('|') ? ' stack' : ''}">
      <div class="var-l"><span class="var-id">${id}</span><p>${why}</p></div>
      ${['light', 'dark'].map(th => `<div class="wctx ctx-${ctx}" data-theme="${th}">${ctx === 'island' ? `<span class="ctx-cap">${th === 'light' ? 'over a light app' : 'over a dark app'}</span>` : ''}${keys.split(',').map(kc => { const [k, cap] = kc.split('|'); return `<div class="wg">${cap ? `<p class="wg-cap">${cap}</p>` : ''}${W[k]()}</div>`; }).join('')}</div>`).join('')}
    </div>`).join('')}</section>`).join('');
})();

/* Calibre · S14 Accounts, S16 Project detail, S17 Computers. S15 Account detail lives in calibre.js. Times from "now" = Wed 25 Nov 21:22. */

const ACCTS = [
  // group, name, tool, plan, week left (null = no window), line, sub windows [label, left], flags
  ['Use next', 'claude-Resal-Dev01', 'claude', 'Max 5x', .64, '64% erased Sat 18:58', [['5-hour', .52], ['Fable', 1]], 'next'],
  ['Low', 'claude-main', 'claude', 'Max 20x', .11, 'Resets Fri 14:58 · ≈ lasts at this pace', [['Fable', .30]], 'low'],
  ['Low', 'sabushanab-resal', 'claude', 'Max 5x', .29, 'Resets Sat 00:58', [['5-hour', .93], ['Fable', .61]], ''],
  ['Room to spare', 'samykabu-teksa', 'claude', 'Max 20x', .75, 'Resets Tue 1 Dec 09:58', [['5-hour', .99], ['Fable', .87]], ''],
  ['Room to spare', 'codex-main', 'codex', 'ChatGPT Pro', .93, 'Resets Tue 1 Dec 17:58 · read just now', [], ''],
  ['Room to spare', 'service-account-resal', 'claude', 'Max 5x', 1, 'Resets Wed 2 Dec 07:58 · idle 2 days', [['Fable', 1]], ''],
  ['Room to spare', 'agy-main', 'agy', 'Google', 1, 'Resets Wed 2 Dec 21:16 · unused this week', [['5-hour', 1]], ''],
  ['No limit window', 'opencode-main', 'opencode', '4 providers', null, 'No limits reported · no use this week', [], ''],
  ['No limit window', 'Claude-Resal-API', 'claude', 'API key', null, 'Pay per use · 98 k tokens this week', [], ''],
];
function accounts() {
  let last = '';
  const rows = ACCTS.map(([g, n, t, plan, w, line, subs, f]) => {
    const head = g !== last ? `${last ? '</div>' : ''}<h3 class="sub-h pad">${g}</h3><div class="card rows ac">` : ''; last = g;
    const d = w === null ? `<span class="ac-dash" aria-label="unknown">—</span>` : `${dial(52, w, { w: 5, ticks: 0, cls: f === 'low' ? 'amber' : '', label: `${Math.round(w * 100)}% of the week left` })}<b class="serif-num">${Math.round(w * 100)}</b>`;
    return `${head}<button class="ac-row" data-to="${n === 'claude-Resal-Dev01' ? 'account' : 'account'}">
      <span class="ac-dial">${d}</span>
      <span class="ac-main"><span class="ac-n">${tool(t)}<b>${n}</b>${f === 'next' ? `<span class="chip chip-xs chip-ink">${ic('push-pin', 1)}Use next</span>` : ''}${f === 'low' ? `<span class="chip chip-xs chip-amber">${ic('warning')}Low</span>` : ''}</span>
        <span class="ac-l">${w === null ? '' : 'Week · '}${line}</span>
        <span class="ac-subs">${subs.map(([l, v]) => `<span>${l}<i class="mini-bar"><i style="width:${v * 100}%"></i></i>${Math.round(v * 100)}%</span>`).join('')}<span class="ac-plan">${plan}</span></span></span>
    </button>`;
  }).join('') + '</div>';
  return `${chrome()}<div class="scroll">
    ${largeTitle('Accounts', '', 2)}
    <div class="filters" role="toolbar" aria-label="Tools"><button class="chip chip-ink">All 9</button><button class="chip">${tool('claude')}Claude 6</button><button class="chip">${tool('codex')}Codex 1</button><button class="chip">${tool('opencode')}OpenCode 1</button><button class="chip">${tool('agy')}Antigravity 1</button></div>
    <p class="ac-age"><span class="age">${ic('clock-counter-clockwise')}Limits read 8 m ago from DEVELOPMENTPC</span><span class="cap">Dial: week left</span></p>
    ${rows}
    <p class="fine">Logins are renewed on the computer, never on the phone. A reading older than an hour is marked with its age.</p>
  </div>${tabbar('Accounts')}`;
}

function project() {
  const d30 = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1.2, 3.8, 6.1, 9.4, 4.2, 11.8, 14.6, 8.3, 7.9, 16.2, 21.4, 12.7, 29.1, 34.3];
  const mx = Math.max(...d30), by = [['Opus 5.5', .51, 'claude'], ['gpt-6.2', .38, 'codex'], ['Fable', .11, 'claude']];
  return `${chrome()}<div class="scroll">
    ${navbar('Sessions', '', needsPill(2))}
    <header class="acc-head"><h1 class="pj-h">HarnessManager-rc-chat</h1>
      <div class="sub">49 sessions · active now · DEVELOPMENTPC and LAPTOP</div></header>
    <div class="sec tight seg-box"><div class="tabs" role="tablist" aria-label="Range"><button role="tab" aria-selected="false">7 days</button><button role="tab" aria-selected="false">14 days</button><button role="tab" aria-selected="true">30 days</button></div></div>
    <div class="sec tight"><div class="card stat3">
      <div><span class="cap">Tokens</span><b class="serif-num">181 M</b></div><div><span class="cap">Cost, list price</span><b class="serif-num">≈ $59</b></div><div><span class="cap">Running</span><b class="serif-num">2</b></div></div></div>
    <section class="sec tight"><div class="card pj-chart">
      <div class="spark tall" aria-label="Tokens per day, last 30 days">${d30.map((d, i) => `<span class="${i === 29 ? 'now' : ''}" style="--h:${Math.max(d ? 3 : 1.5, d / mx * 100).toFixed(1)}"></span>`).join('')}</div>
      <div class="pj-axis"><span>27 Oct</span><span>first session 11 Nov</span><span>today 34 M</span></div></div></section>
    <section class="sec"><h3>By model</h3><div class="card rows bym">${by.map(([m, v, t]) => `<div class="bm"><span>${tool(t)}${m}</span><div class="bar"><span style="--v:${v}"></span></div><b>${Math.round(v * 100)}%</b></div>`).join('')}
      <p class="cap" style="margin:4px 0 12px">Tap a bar for the requests behind it: next version.</p></div></section>
    <section class="sec"><h3>Sessions</h3><div class="card rows ss">${[RUNNING[0], RUNNING[2], EARLIER[2]].map((r, i) => sRow(r, i)).join('')}</div></section>
  </div>${tabbar('Sessions')}`;
}

const route = hub => `<span class="route" aria-hidden="true">${ic('device-mobile')}<i class="ln"></i>${hub ? `${ic('cloud')}<i class="ln"></i>` : ''}${ic('desktop-tower')}</span>`;
function computers() {
  const g = (on) => ['Summaries', 'Transcripts', 'Control'].map((n, i) => `<span class="g${on[i] ? ' on' : ''}">${on[i] ? ic('check') : ic('minus')}${n}</span>`).join('');
  return `${chrome()}<div class="scroll">
    ${largeTitle('Computers', '', 2, `<button class="req-btn" aria-label="Pair another computer">${ic('plus')}</button>`)}
    <div class="comps">
      <article class="card comp">
        <div class="cp-h"><span class="cp-ic">${ic('desktop-tower')}</span><div><b>DEVELOPMENTPC</b><small>Windows 11 · Harness Manager 1.14.0</small></div></div>
        <p class="cp-route">${route(false)}This network, direct · pinned</p>
        <dl class="cp-kv"><div><dt>Last contact</dt><dd>just now</dd></div><div><dt>Running</dt><dd>5</dd></div><div><dt>Accounts</dt><dd>9</dd></div></dl>
        <div class="grants">${g([1, 1, 1])}</div>
      </article>
      <article class="card comp">
        <div class="cp-h"><span class="cp-ic">${ic('laptop')}</span><div><b>LAPTOP</b><small>Windows 11 · Harness Manager 1.14.0</small></div></div>
        <p class="cp-route">${route(true)}Through your hub · sealed to this phone</p>
        <dl class="cp-kv"><div><dt>Last contact</dt><dd>3 m ago</dd></div><div><dt>Running</dt><dd>2</dd></div><div><dt>Accounts</dt><dd>4</dd></div></dl>
        <div class="grants">${g([1, 1, 1])}</div>
      </article>
      <article class="card comp asleep">
        <div class="cp-h"><span class="cp-ic">${ic('moon')}</span><div><b>MINI</b><small>macOS · Harness Manager 1.14.0</small></div><span class="chip chip-xs">Asleep</span></div>
        <p class="cp-route">${ic('clock-counter-clockwise')}Last seen 2 d ago · showing its snapshot, actions off</p>
        <dl class="cp-kv"><div><dt>Last contact</dt><dd>Mon 21:40</dd></div><div><dt>Running</dt><dd>—</dd></div><div><dt>Accounts</dt><dd>2</dd></div></dl>
        <div class="grants">${g([1, 1, 0])}</div>
      </article>
    </div>
    <h3 class="sub-h pad">Known, not granted</h3>
    <div class="card empty-row">${ic('check-circle')}<p>None. Every computer your computers sync with has granted this phone. New ones appear here with an Ask button.</p></div>
    <button class="btn btn-quiet btn-sm arrange">${ic('qr-code')}Pair another computer</button>
    <p class="fine">Grants are changed on each computer, in Computers › Phones. A phone out of touch for 14 days loses its grants on its own.</p>
  </div>${tabbar('Computers')}`;
}

Object.assign(SCREENS, { accounts, project, computers });

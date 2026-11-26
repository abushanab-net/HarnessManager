/* Calibre · onboarding and pairing, S1 to S6. No account to sign up for: the phone registers with the owner's computers. */

/* the app icon: a night dial with a lume arc and a hand. Fixed colours, like any app icon */
function appIcon(S = 96) {
  const c = S / 2, r = S * .3, C = 2 * Math.PI * r;
  return `<svg class="dial app-icon" width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="Harness Manager app icon">
    <rect width="${S}" height="${S}" rx="${S * .225}" fill="#0b1120"/>
    <g style="--ink-3:#8a92a3;--ink:#e9ece6">${tickRing(c, c, S * .42, 60, S * .025, 5, S * .05)}</g>
    <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="#1d2840" stroke-width="${S * .07}"/>
    <circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="#6fddb0" stroke-width="${S * .07}" stroke-linecap="round" transform="rotate(-90 ${c} ${c})" stroke-dasharray="${(C * .64).toFixed(2)} ${C.toFixed(2)}"/>
    <line x1="${c}" y1="${c}" x2="${pt(c, c, r * .78, 212)[0]}" y2="${pt(c, c, r * .78, 212)[1]}" stroke="#e9ece6" stroke-width="${S * .035}" stroke-linecap="round"/>
    <circle cx="${c}" cy="${c}" r="${S * .04}" fill="#e9ece6"/></svg>`;
}

function welcome() {
  return `${chrome()}<div class="scroll ob welcome" style="padding-bottom:40px">
    <div class="wl-art">${dial(236, .64, { w: 3, label: '' })}<span class="wl-icon">${appIcon(96)}</span></div>
    <h1>Harness Manager</h1>
    <p class="lead">Your AI coding accounts, sessions and limits, on your phone.</p>
    <div class="choices">
      <button class="choice" data-to="scan"><span class="ci">${ic('qr-code')}</span><span><b>Pair with a computer</b><small>Scan the code in Computers › Pair a phone</small></span>${ic('caret-right')}</button>
      <button class="choice" data-to="scan"><span class="ci">${ic('globe-hemisphere-west')}</span><span><b>Connect through my hub</b><small>Away from home: scan a hub invitation</small></span>${ic('caret-right')}</button>
    </div>
    <p class="fine c">No account to sign up for. This phone registers with your own computers, which need Harness Manager 1.14 or later.</p>
    <button class="link-btn">How it works</button>
  </div>`;
}

/* a QR pattern for the mock camera, deterministic */
function qr(n = 25, cell = 5) {
  let s = '', x = 7;
  const finder = (i, j) => (i < 7 && j < 7) || (i < 7 && j >= n - 7) || (i >= n - 7 && j < 7);
  for (let i = 0; i < n; i++) for (let j = 0; j < n; j++) {
    let on;
    if (finder(i, j)) { const a = i % (n - 7), b = j % (n - 7), ii = i < 7 ? i : i - (n - 7), jj = j < 7 ? j : j - (n - 7);
      on = ii === 0 || ii === 6 || jj === 0 || jj === 6 || (ii > 1 && ii < 5 && jj > 1 && jj < 5); void a; void b; }
    else { x = (x * 1103515245 + 12345) & 0x7fffffff; on = (x >> 16) % 2 === 0; }
    if (on) s += `<rect x="${j * cell}" y="${i * cell}" width="${cell}" height="${cell}"/>`;
  }
  return `<svg width="${n * cell}" height="${n * cell}" viewBox="0 0 ${n * cell} ${n * cell}" aria-hidden="true">${s}</svg>`;
}
function scan() {
  const sq = 236;
  let ticks = '';                                    // minute ticks along the viewfinder edge: the dial language on a square
  for (let i = 1; i < 12; i++) { const p = sq * i / 12; ticks += `<line x1="${p}" y1="0" x2="${p}" y2="${i % 3 ? 5 : 9}"/><line x1="${p}" y1="${sq}" x2="${p}" y2="${sq - (i % 3 ? 5 : 9)}"/><line x1="0" y1="${p}" x2="${i % 3 ? 5 : 9}" y2="${p}"/><line x1="${sq}" y1="${p}" x2="${sq - (i % 3 ? 5 : 9)}" y2="${p}"/>`; }
  return `${chrome()}<div class="cam" aria-label="Camera">
      <div class="cam-scene"><div class="cam-screen"><span>Pair a phone</span>${qr()}<em>481 920</em></div></div>
      <div class="cam-top"><button class="cam-x" data-back aria-label="Close">${ic('x')}</button><b>Scan the code</b><span></span></div>
      <button class="vf" data-to="approve" aria-label="Scanning. In the prototype, tap to simulate a scan.">
        <svg width="${sq}" height="${sq}" viewBox="0 0 ${sq} ${sq}" aria-hidden="true"><g class="vf-t">${ticks}</g>
          <path class="vf-c" d="M0 34V0h34M${sq - 34} 0h34v34M${sq} ${sq - 34}v34h-34M34 ${sq}H0v-34"/></svg></button>
      <p class="cam-hint">Hold the code inside the frame</p>
    </div>
    <section class="cam-sheet">
      <span class="grabber" aria-hidden="true"></span>
      <p class="cs-k">On your computer: <b>Computers › Pair a phone</b></p>
      <button class="found" data-to="code"><span class="ci">${ic('desktop-tower')}</span><span><b>DEVELOPMENTPC</b><small>Found on this network, pairing open</small></span><span class="chip">Use code</span></button>
      <button class="btn btn-quiet btn-sm wide" data-to="code">${ic('keyboard')}Type the code instead</button>
      <p class="fine c" style="margin:10px 0 0">Camera access is asked now, the first time, and only for this.</p>
    </section>`;
}

function code() {
  const keys = [['1', ''], ['2', 'ABC'], ['3', 'DEF'], ['4', 'GHI'], ['5', 'JKL'], ['6', 'MNO'], ['7', 'PQRS'], ['8', 'TUV'], ['9', 'WXYZ'], ['', ''], ['0', ''], ['del', '']];
  return `${chrome()}<div class="scroll ob" style="padding-bottom:0">
    ${navbar('Scan')}
    <h1 class="ob-h">Enter the code</h1>
    <p class="lead">The six digits on DEVELOPMENTPC, under Computers › Pair a phone.</p>
    <div class="code" role="group" aria-label="Code, 3 of 6 digits">
      <span>4</span><span>8</span><span>1</span><i></i><span class="cur"></span><span></span><span></span></div>
    <div class="code-meta">${countdown(28, 102, 120, 'pc')}<span><b data-left="pc" data-start="102">1:42</b> left · 3 tries</span></div>
    <label class="field-l">Computer</label>
    <button class="field">${ic('desktop-tower')}<span><b>DEVELOPMENTPC</b><small>found on this network</small></span>${ic('caret-up-down')}</button>
    <button class="link-btn left">Or type an address, host:port</button>
  </div>
  <div class="kb-wrap">
    <button class="btn btn-primary wide" disabled>Continue</button>
    <div class="numpad" aria-hidden="true">${keys.map(([k, l]) => k === 'del' ? `<span class="k fn">${ic('backspace')}</span>` : k ? `<span class="k"><b>${k}</b><small>${l}</small></span>` : '<span class="k fn"></span>').join('')}</div>
  </div>`;
}

const GRANTS = [['chart-donut', 'Summaries', 'Limits, sessions and usage'], ['article', 'Transcripts', 'What each session said and did'], ['hand-tap', 'Control', 'Answer, reply and Stop']];
function approve() {
  return `${chrome()}<div class="scroll ob" data-auto="connected:3200">
    ${navbar('Cancel')}
    <h1 class="ob-h">Check your computer</h1>
    <p class="lead">DEVELOPMENTPC should show these same three words. If they differ, cancel there.</p>
    <div class="words"><span>amber</span><span>otter</span><span>nine</span></div>
    <h3 class="ob-k">This phone asks for</h3>
    <div class="card list">${GRANTS.map(([i, n, d]) => `<div class="li">${ic(i)}<span><b>${n}</b><small>${d}</small></span><span class="cap">Asked</span></div>`).join('')}</div>
    <p class="fine">Your computer has the final say, and you can change it there later.</p>
  </div>
  <div class="actions">
    <div class="timer">
      <span style="width:40px;height:40px">${countdown(40, 91, 120, 'pa')}</span>
      <div><div class="left">Waiting for approval</div><p>Open on DEVELOPMENTPC for <b data-left="pa" data-start="91">1:31</b> more</p></div>
    </div>
    <button class="btn btn-secondary wide" data-back>Cancel pairing</button>
  </div>`;
}

function connected() {
  const n = (v, l) => `<div><b class="serif-num" data-count="${v}">${v}</b><span>${l}</span></div>`;
  return `${chrome()}<div class="scroll ob" style="padding-bottom:200px">
    <div class="seal big">${dial(96, 1, { w: 4, cls: 'lume', ticks: 60 })}${ic('check')}</div>
    <h1 class="ob-h">Connected to DEVELOPMENTPC</h1>
    <p class="lead">${ic('wifi-high')} This network, direct. Its certificate is pinned.</p>
    <div class="card sync">${n(9, 'accounts')}${n(7, 'running sessions')}${n(3, 'computers known')}</div>
    <h3 class="ob-k">Granted by DEVELOPMENTPC</h3>
    <div class="card list">${GRANTS.map(([i, nm]) => `<div class="li">${ic(i)}<span><b>${nm}</b></span><span class="on">${ic('check-circle', 1)}On</span></div>`).join('')}</div>
    <h3 class="ob-k">It knows two more computers</h3>
    <div class="card list">
      <div class="li">${ic('laptop')}<span><b>LAPTOP</b><small>Reachable through your hub</small></span><button class="chip chip-tap">Ask for access</button></div>
      <div class="li dim">${ic('moon')}<span><b>MINI</b><small>Asleep for 2 days</small></span><span class="cap">Ask when awake</span></div>
    </div>
  </div>
  <div class="actions"><button class="btn btn-primary wide" data-to="notify">Continue</button>
    <button class="btn btn-quiet btn-sm wide" style="margin-top:8px">Pair another computer</button></div>`;
}

function notify() {
  return `${chrome()}<div class="scroll ob" style="padding-bottom:200px">
    <div class="ls-art" aria-hidden="true"><div class="ls-time">21:22</div><div class="ls-date">Wednesday 25 November</div>
      <div class="ls-note"><span class="ls-ic">${appIcon(30)}</span><span><b>Harness Manager</b><span>A session needs you</span></span><em>now</em></div></div>
    <h1 class="ob-h">Know when a session needs you</h1>
    <div class="card list">
      <div class="li">${ic('chat-circle-dots')}<span><b>A question or permission is waiting</b><small>with the time left to answer</small></span></div>
      <div class="li">${ic('plant')}<span><b>A window resets with room left</b><small>a harvest you can still make</small></span></div>
      <div class="li">${ic('key')}<span><b>A login expired</b><small>renew it on the computer</small></span></div>
    </div>
    <p class="private">${ic('lock-simple')}<span>Your lock screen only says “A session needs you”. Details load after you unlock.</span></p>
  </div>
  <div class="actions"><button class="btn btn-primary wide" data-to="home">Allow notifications</button>
    <button class="btn btn-quiet btn-sm wide" style="margin-top:8px" data-to="home">Not now</button></div>`;
}

Object.assign(SCREENS, { welcome, scan, code, approve, connected, notify });

/* the first-sync numbers count up once when Connected opens */
document.addEventListener('calibre:mount', () => setTimeout(() => document.querySelectorAll('[data-count]').forEach(el => countUp(el, 0, +el.dataset.count, 600)), 300));
document.addEventListener('proto:nav', e => e.detail.ph.querySelectorAll('[data-count]').forEach(el => countUp(el, 0, +el.dataset.count, 600)));

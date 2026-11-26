/* Calibre · S8 Requests inbox, S10 Question, and the Needs-you entry board. S7 Home and S9 Permission live in calibre.js. */

const receipt = rows => `<section class="receipt" aria-live="polite">
    <div class="seal">${dial(96, 1, { w: 4, cls: 'lume', ticks: 60 })}${ic('check', 0)}</div>
    <h2 data-rt>Sent</h2><p data-rp></p>
    <div class="card rows">${rows.map(([k, v]) => `<div><span>${k}</span><span>${v}</span></div>`).join('')}</div>
    <button class="btn btn-primary done" data-reset data-to="home">Back to Home</button>
  </section>`;

function requests(v = '') {
  const resolved = [
    ['desktop', 'Edit app/ui/accounts.js', 'Answered in the terminal', '12 m ago'],
    ['device-mobile', 'cargo build --release', 'You approved once, Face ID', '1 h ago'],
    ['hourglass-low', 'Which test runner should I use?', 'Went back to the terminal after 60 s', '2 h ago'],
    ['device-mobile', 'git push origin develop', 'You denied it', '3 h ago'],
    ['desktop', 'Write crates/core/src/sync/peer.rs', 'Answered in the terminal', '4 h ago'],
    ['hourglass-low', 'rm -rf target/debug/incremental', 'Closed after 5 min unanswered', '6 h ago'],
  ];
  const row = ([i, t, w, a], mono) => `<div class="rs-row"><span class="rs-i">${ic(i)}</span><span><b class="${mono ? 'mono' : ''}">${t}</b><small>${w}</small></span><span class="cap">${a}</span></div>`;
  const isMono = t => !t.includes('?');
  return `${chrome()}<div class="scroll">
    ${navbar('Home', '', 'gear-six')}
    <header class="lt" style="padding-top:0"><h1>Requests</h1></header>
    <div class="sec tight seg-box" style="padding-top:6px">
      <div class="tabs" role="tablist" aria-label="Requests"><button role="tab" aria-selected="true" data-tab-to="open">Open <small data-needs>2</small></button><button role="tab" aria-selected="false" data-tab-to="done">Resolved <small>6</small></button></div>
      <div class="tabpanel" data-panel="open">
        <button class="card rq" data-to="permission">
          <span class="rq-h"><span class="ico">${ic('terminal')}</span><span class="what">Permission<br><small>run a command</small></span>
            <span class="clock">${countdown(40, 252, 300, 'p1')}<span><b data-left="p1" data-start="252">4:12</b><small>of 5:00</small></span></span></span>
          <code class="rq-cmd">npm run test -- --watch=false</code>
          <span class="rq-m"><b>Account limit re-login notifications</b><span>${tool('claude')}sabushanab-resal · DEVELOPMENTPC</span></span>
          <span class="rq-go">Review and answer ${ic('caret-right')}</span>
        </button>
        <button class="card rq" data-req="q1" data-to="question">
          <span class="rq-h"><span class="ico">${ic('chat-circle-text')}</span><span class="what">Question<br><small>pick one</small></span>
            <span class="clock">${countdown(40, 41, 60, 'q1')}<span><b data-left="q1" data-start="41">0:41</b><small>of 1:00</small></span></span></span>
          <span class="rq-q">Which branch should I base this on?</span>
          <span class="rq-opts"><span>develop</span><span>main</span><span>feature/speckit-29</span></span>
          <span class="rq-m"><b data-qmeta>Bunyan · LAPTOP, through your hub</b><span>${tool('claude')}samykabu-teksa</span></span>
          <span class="rq-go">Answer ${ic('caret-right')}</span>
        </button>
        <p class="fine" style="margin:12px 4px 0">${ic('moon')} MINI is asleep: anything it asks waits in its terminal.</p>
        <h3 class="sub-h">Earlier today</h3>
        <div class="card rows rs">${resolved.slice(0, 3).map(r => row(r, isMono(r[1]))).join('')}</div>
      </div>
      <div class="tabpanel" data-panel="done" hidden>
        <div class="card rows rs">${resolved.map(r => row(r, isMono(r[1]))).join('')}</div>
      </div>
    </div>
    <p class="fine">Works without notifications: this list reads every computer you can reach, direct or through your hub. Each resolved request says who answered.</p>
  </div>${tabbar('Home')}`;
}

function question(v = '') {
  const exp = v === 'expired';
  const opt = (n, d, on) => `<button role="radio" aria-checked="${on}" data-pick><span class="rd" aria-hidden="true"></span><span><b class="${n.includes('/') || n === 'main' || n === 'develop' ? 'mono' : ''}">${n}</b><small>${d}</small></span></button>`;
  return `${chrome()}<div class="scroll${exp ? ' expired' : ''}" style="padding-bottom:210px" data-qscreen>
    ${navbar('Home', 'Question')}
    <div class="perm">
      <div class="who">${tool('claude')}Claude Code · samykabu-teksa</div>
      <h2>Which branch should I base this on?</h2>
      <p class="sess">Bunyan · LAPTOP, through your hub</p>
      <div class="opts" role="radiogroup" aria-label="Answers">
        ${opt('develop', 'The integration branch', true)}${opt('main', 'What is released', false)}${opt('feature/speckit-29', "Where issue 29's spec lives", false)}
        <button role="radio" aria-checked="false" data-pick class="own"><span class="rd" aria-hidden="true"></span><span><b>Write your own</b><small>One line, sent as the answer</small></span></button>
      </div>
      <p class="fine" style="margin:8px 2px 0">Claude Code offered these three and asks for one.</p>
      <section class="ctx card" aria-label="What led to this question">
        <blockquote class="last" style="border:0;margin:0;padding:0"><span class="k">Claude, just before asking</span>
          <p>“Issue 29's spec is ready. Before I branch, which base do you want?”</p></blockquote>
      </section>
    </div>
  </div>
  <div class="actions">
    <div class="timer">
      <span style="width:40px;height:40px">${countdown(40, exp ? 0 : 41, 60, exp ? 'qx' : 'q1')}</span>
      <div><div class="left"><b class="serif-num" ${exp ? '' : 'data-left="q1" data-start="41"'}>${exp ? '0:00' : '0:41'}</b> to answer</div>
        <p>Then it waits in the terminal on LAPTOP. You get 60 s because this phone has Control.</p></div>
    </div>
    <button class="btn btn-primary wide" data-done data-title="Sent: develop" data-text="LAPTOP confirmed it at 21:22:31, through your hub. Claude Code is branching from develop.">${ic('paper-plane-tilt')}Send answer</button>
  </div>
  <section class="q-expired" aria-live="polite">
    <div class="seal">${dial(96, 0, { w: 4, ticks: 60 })}${ic('hourglass-low')}</div>
    <h2>Back in the terminal</h2>
    <p>No answer within 60 s, so the question went back to the terminal on LAPTOP at 21:23. Nothing was sent. Answer it there, or reply to the session when it asks again.</p>
    <div class="row2"><button class="btn btn-secondary" data-to="session">Open session</button><button class="btn btn-primary" data-to="home">Back to Home</button></div>
  </section>
  ${receipt([['Answer', '<span class="mono">develop</span>'], ['Answered by', 'This phone'], ['Delivered', 'Through your hub · 380 ms'], ['Session', 'Use /speckit-workflow-c…']])}`;
}

Object.assign(SCREENS, { requests, question });

/* the Needs-you entry on every tab: the top of each tab, light and dark, plus the quiet state with nothing waiting */
(function entryBoard() {
  const el = document.querySelector('[data-entry]'); if (!el) return;
  const crops = [['home', 'Home'], ['sessions', 'Sessions'], ['accounts', 'Accounts'], ['computers', 'Computers'], ['you', 'You'], ['sessions', 'Nothing waiting', 'quiet']];
  el.innerHTML = ['light', 'dark'].map(th => `<div class="crop-row">${crops.map(([k, n, v]) =>
    `<figure><div class="crop-frame"><div class="phone crop" data-theme="${th}" data-screen="${k}" ${v ? `data-v="${v}"` : ''} data-static></div></div><figcaption>${n}</figcaption></figure>`).join('')}</div>`).join('');
})();

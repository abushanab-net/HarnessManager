/* Calibre · You tab: the You root, S19 Customize Home, S20 Settings and privacy, S21 Notification settings. S18 Progress lives in calibre.js. */

const sw = (on, label, dis) => `<button class="sw-t" role="switch" aria-checked="${on}" aria-label="${label}" ${dis ? 'disabled' : 'data-toggle'}><i></i></button>`;
const set = (icon, label, small, ctl) => `<div class="set">${icon ? `<span class="s-i">${ic(icon)}</span>` : ''}<span class="s-t"><b>${label}</b>${small ? `<small>${small}</small>` : ''}</span>${ctl}</div>`;
const more = v => `<span class="s-v">${v}${ic('caret-right')}</span>`;
const group = (title, rows, foot) => `<section class="grp">${title ? `<h3>${title}</h3>` : ''}<div class="card">${rows.join('')}</div>${foot ? `<p class="grp-f">${foot}</p>` : ''}</section>`;

function you() {
  return `${chrome()}<div class="scroll">
    ${largeTitle('You', '', 2)}
    <button class="card you-lvl" data-to="progress" aria-label="Progress: Planner, 1,240 of 1,500 XP, 12-day streak">
      <span class="g-gauge">${powerReserve(64, 1240 / 1500, true)}</span>
      <span class="yl-t"><b>Planner</b><span>1,240 of 1,500 XP · 7 badges</span></span>
      <span class="datewheel" aria-hidden="true"><span class="dw-win"><span class="dw-col"><i>11</i><b>12</b><i>13</i></span></span><span class="dw-l">day<br>streak</span></span>
      ${ic('caret-right')}
    </button>
    ${group('', [`<button class="set" data-to="progress"><span class="s-i">${ic('medal')}</span><span class="s-t"><b>Progress</b><small>Level, streak, badges, weekly recap</small></span>${more('')}</button>`,
      `<button class="set" data-to="customize"><span class="s-i">${ic('squares-four')}</span><span class="s-t"><b>Customize Home</b><small>Arrange, resize and add widgets</small></span>${more('')}</button>`,
      `<button class="set"><span class="s-i">${ic('device-mobile')}</span><span class="s-t"><b>Home and Lock Screen widgets</b><small>How to add them</small></span>${more('')}</button>`])}
    ${group('', [`<button class="set" data-to="settings"><span class="s-i">${ic('shield-check')}</span><span class="s-t"><b>Settings and privacy</b></span>${more('')}</button>`,
      `<button class="set" data-to="notifs"><span class="s-i">${ic('bell-simple')}</span><span class="s-t"><b>Notifications</b></span>${more('5 on')}</button>`,
      `<button class="set"><span class="s-i">${ic('circle-half')}</span><span class="s-t"><b>Appearance</b></span>${more('System')}</button>`])}
    ${group('', [`<button class="set"><span class="s-i">${ic('question')}</span><span class="s-t"><b>How pairing and privacy work</b></span>${more('')}</button>`,
      `<button class="set"><span class="s-i">${ic('chat-teardrop-text')}</span><span class="s-t"><b>Send feedback</b></span>${more('')}</button>`],
      'Harness Manager for iPhone 1.0 · paired with 3 computers since 9 Nov')}
  </div>${tabbar('You')}`;
}

/* small previews for the widget gallery (in-app board and Home Screen share one catalogue) */
function miniPreview(k) {
  if (k === 'clock') return `<span class="mp">${dial(64, .64, { w: 5, ticks: 30 })}<b class="serif-num">64</b></span>`;
  if (k === 'rings') return `<span class="mp rings4">${[.64, .29, .11, .93].map((v, i) => dial(34, v, { w: 3.5, ticks: 0, cls: i === 2 ? 'amber' : '' })).join('')}</span>`;
  if (k === 'cost') return `<span class="mp txt"><b class="serif-num">≈ $737</b><small>today</small></span>`;
  if (k === 'comps') return `<span class="mp txt"><small>DEV PC 5 · LAPTOP 2</small><small>MINI asleep</small></span>`;
  if (k === 'streak') return `<span class="mp"><span class="datewheel big" aria-hidden="true"><span class="dw-win"><span class="dw-col"><i>11</i><b>12</b><i>13</i></span></span></span></span>`;
  return `<span class="mp harv">${ic('plant')}<small>Sat 18:58</small></span>`;
}
function customize() {
  const on = [['bell-ringing', 'Needs you', 'L'], ['gauge', 'Use next', 'M'], ['terminal-window', 'Running', 'S'], ['timer', 'Next reset', 'S'], ['chart-bar', 'Today', 'M']];
  const add = [['clock', 'Reset clock', 'Limits'], ['rings', 'Week rings', 'Limits'], ['harvest', 'Harvest window', 'Progress'], ['streak', 'Streak', 'Progress'], ['cost', 'Cost today', 'Usage'], ['comps', 'Computers', 'Sessions']];
  return `${chrome()}<div class="scroll">
    ${navbar('You', 'Customize Home', '<button class="nb-done" data-back>Done</button>')}
    <h3 class="sub-h pad" style="margin-top:6px">On Home <span>drag to reorder</span></h3>
    <div class="card rows cz">${on.map(([i, n, s], k) => `<div class="cz-row${k === 1 ? ' sel' : ''}"><span class="cz-g" aria-label="Move ${n}">${ic('dots-six-vertical')}</span>${ic(i)}<b>${n}</b>
      <span class="sizes" role="radiogroup" aria-label="${n} size">${['S', 'M', 'L'].map(z => `<button role="radio" aria-checked="${z === s}" data-pick>${z}</button>`).join('')}</span>
      <button class="cz-x" aria-label="Remove ${n}">${ic('minus-circle')}</button></div>`).join('')}</div>
    <h3 class="sub-h pad">Add a widget</h3>
    <div class="filters" role="toolbar" aria-label="Categories"><button class="chip chip-ink">All</button><button class="chip">Limits</button><button class="chip">Sessions</button><button class="chip">Progress</button><button class="chip">Usage</button></div>
    <div class="gal">${add.map(([k, n, c]) => `<div class="card gt"><div class="gt-p">${miniPreview(k)}</div><div class="gt-f"><span><b>${n}</b><small>${c} · S M L</small></span><button class="gt-add" aria-label="Add ${n}">${ic('plus')}</button></div></div>`).join('')}</div>
    <p class="fine">One catalogue for this board and for your iPhone Home Screen and Lock Screen. There, touch and hold the Home Screen, then Edit.</p>
  </div>`;
}

function settings() {
  return `${chrome()}<div class="scroll">
    ${navbar('You', 'Settings and privacy')}
    ${group('Security', [set('scan', 'Face ID to open', 'Ask every time the app opens', sw(false, 'Face ID to open')),
      set('fingerprint', 'Face ID to approve', 'Always on while a computer grants Control', sw(true, 'Face ID to approve', true))])}
    ${group('Privacy', [set('lock-simple', 'Notification detail', 'What the lock screen may show', more('Hidden')),
      set('squares-four', 'Widget detail', 'Names and commands in widgets', more('Redacted')),
      set('device-mobile', 'Lock Screen detail', 'Session titles in Live Activities and the ongoing notification', sw(false, 'Lock Screen detail')),
      set('article', 'Keep transcripts on this phone', '', more('7 days')),
      set('export', 'Recap image shows the busiest project', '', sw(true, 'Recap shows the busiest project'))])}
    ${group('Access, granted by each computer', [set('desktop-tower', 'DEVELOPMENTPC', 'Summaries, Transcripts, Control', more('')),
      set('laptop', 'LAPTOP', 'Summaries, Transcripts, Control', more('')),
      set('moon', 'MINI', 'Summaries, Transcripts', more(''))], 'Grants are changed on the computer. This phone only asks.')}
    ${group('', [`<button class="set" data-to="notifs"><span class="s-i">${ic('bell-simple')}</span><span class="s-t"><b>Notifications</b></span>${more('5 on')}</button>`,
      set('plant', 'Progress and game', 'XP, streak, badges, recap', sw(true, 'Game layer')),
      set('circle-half', 'Appearance', '', more('System'))])}
    <section class="grp"><div class="card"><button class="set danger"><span class="s-i">${ic('sign-out')}</span><span class="s-t"><b>Disconnect this phone</b></span></button></div>
      <p class="grp-f">Erases this phone's key and cache and tells every computer. Pairing again makes a new key.</p></section>
  </div>`;
}

function notifs() {
  return `${chrome()}<div class="scroll">
    ${navbar('Settings', 'Notifications')}
    ${group('Events', [set('', 'A session needs you', 'Questions and permissions. The lock screen says only “A session needs you”.', sw(true, 'A session needs you')),
      set('', 'A session finished', '', sw(false, 'A session finished')),
      set('', 'Login expired', '', sw(true, 'Login expired')),
      set('', 'Limit low', '<button class="inl">Below 20% ›</button> per account', sw(true, 'Limit low')),
      set('', 'Harvest reminder', '<button class="inl">30 m before ›</button> a reset with room left', sw(true, 'Harvest reminder')),
      set('', 'Streak at risk', '', sw(false, 'Streak at risk')),
      set('', 'Weekly recap', 'Sundays at 18:00', sw(true, 'Weekly recap'))])}
    ${group('Quiet hours', [set('moon-stars', 'Quiet hours', '23:00 to 07:00', sw(true, 'Quiet hours')),
      set('bell-ringing', 'Requests still come through', 'A waiting session is worth waking for', sw(true, 'Requests break quiet hours'))])}
    <section class="grp"><div class="card test"><button class="btn btn-secondary btn-sm wide">${ic('paper-plane-tilt')}Send a test notification</button>
      <p class="cap">Last delivered 2 m ago through the relay, 1.2 s end to end.</p></div></section>
  </div>`;
}

Object.assign(SCREENS, { you, customize, settings, notifs });

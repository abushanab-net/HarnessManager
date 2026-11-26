/* Calibre · the empty / first-sync Home and S23 (access removed, update needed). S22 is home('asleep' | 'hub' | 'offline') in calibre.js. */

const skel = (w, h = 10) => `<i class="sk" style="width:${w};height:${h}px"></i>`;
function homeEmpty() {
  return `${chrome()}<div class="scroll">
    ${largeTitle('Home', 'Wednesday 25 November', 0)}
    <button class="gstrip" data-to="progress" aria-label="Connected, level 1. 0 XP. First day.">
      <span class="g-gauge">${powerReserve(40, 0, true)}</span>
      <span class="g-lvl"><b>Connected</b><span class="g-xp">first harvest: +40 XP</span></span>
      <span></span>
      <span class="datewheel" aria-hidden="true"><span class="dw-win"><span class="dw-col"><i>0</i><b>1</b><i>2</i></span></span><span class="dw-l">day<br>streak</span></span>
    </button>
    <section class="card first-sync">
      <span class="fs-dial">${dial(44, 3 / 9, { w: 4, ticks: 24, label: '3 of 9 accounts read' })}</span>
      <div><b>First sync from DEVELOPMENTPC</b><p>Reading limits: 3 of 9 accounts. Sessions and requests are already live.</p></div>
    </section>
    <div class="board">
      <article class="card span2" aria-label="Needs you">
        <h2 class="w-title">Needs you <span class="serif-num" style="font-size:15px;color:var(--ink)">0</span></h2>
        <div class="empty">${ic('tray')}<p><b>Nothing waiting</b><span>When a session asks a question or wants permission, it shows here with the time left to answer.</span></p></div>
      </article>
      <article class="card span2" aria-label="Use next, loading">
        <h2 class="w-title">Use next <span class="more"><span class="age">reading…</span></span></h2>
        <div class="un" style="pointer-events:none"><div class="dialwrap">${dial(116, 0, { w: 9, label: 'not read yet' })}<div class="c"><b style="color:var(--ink-3)">—</b><span>week left</span></div></div>
          <div class="sk-col">${skel('80%', 14)}${skel('92%', 18)}${skel('60%')}</div></div>
        <p class="cap" style="margin:12px 0 0">The account to use next appears when every limit is read, so the pick is never made on half the data.</p>
      </article>
      <article class="card" aria-label="Running"><h2 class="w-title">Running</h2><div class="sm-big serif-num">7 <small>sessions</small></div><div class="kv">DEVELOPMENTPC 5<br>LAPTOP 2</div></article>
      <article class="card" aria-label="Next reset, loading"><h2 class="w-title">Next reset</h2><div class="sk-col" style="margin-top:14px">${skel('70%', 24)}${skel('86%')}${skel('54%')}</div></article>
    </div>
  </div>${tabbar('Home')}`;
}

function access(v = 'removed') {
  const up = v === 'update';
  return `${chrome()}<div class="scroll ob dead" style="padding-bottom:200px">
    <div class="seal big">${dial(96, 0, { w: 4, ticks: 60 })}${ic(up ? 'arrow-circle-up' : 'lock-key')}</div>
    <h1 class="ob-h">${up ? 'Update Harness Manager on DEVELOPMENTPC' : 'Access removed'}</h1>
    <p class="lead">${up ? 'DEVELOPMENTPC runs Harness Manager 1.13.2. The phone needs 1.14 or later, which adds the companion API it talks to.'
      : 'DEVELOPMENTPC removed this phone on 25 Nov at 17:40. Everything this phone kept from it has been erased.'}</p>
    ${up ? `<h3 class="ob-k">On DEVELOPMENTPC</h3><div class="card list steps-l">
        <div class="li"><span class="n">1</span><span><b>Open Settings › About</b></span></div>
        <div class="li"><span class="n">2</span><span><b>Check for updates</b><small>1.14.0 is out since 24 Nov</small></span></div>
        <div class="li"><span class="n">3</span><span><b>Come back here</b><small>The phone retries by itself</small></span></div></div>`
      : `<h3 class="ob-k">Still connected</h3><div class="card list">
        <div class="li">${ic('laptop')}<span><b>LAPTOP</b><small>Summaries, Transcripts, Control</small></span><span class="on">${ic('check-circle', 1)}Granted</span></div>
        <div class="li dim">${ic('moon')}<span><b>MINI</b><small>Asleep, Summaries and Transcripts</small></span><span class="cap">2 d ago</span></div></div>
        <p class="fine">If you didn't remove it yourself, someone with access to DEVELOPMENTPC did.</p>`}
  </div>
  <div class="actions"><button class="btn btn-primary wide">${up ? "I've updated, try again" : 'Pair DEVELOPMENTPC again'}</button>
    <button class="btn btn-quiet btn-sm wide" style="margin-top:8px">${up ? 'Use LAPTOP meanwhile' : 'Continue with LAPTOP'}</button></div>`;
}

Object.assign(SCREENS, { 'home-empty': homeEmpty, access });

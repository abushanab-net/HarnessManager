/* Calibre · Android adaptations (same Flutter widgets and Calibre tokens, Material 3 structure) and S24, the desktop's side of pairing. */

/* edge-to-edge: a transparent status bar over the content (with a soft top scrim) and the gesture handle over the navigation bar */
const androidChrome = () => `<div class="a-cam"></div>
  <div class="status a-status"><span>21:22</span><span class="r">${ic('wifi-high', 1)}${ic('cell-signal-full', 1)}${ic('battery-high', 1)}<small>82%</small></span></div>
  <div class="a-gesture"></div>`;
/* Material 3 NavigationBar: 80 dp, a 64 × 32 active-indicator pill behind the selected icon, labels always shown */
const m3Nav = active => `<nav class="m3-nav" aria-label="Navigation">${TABS.map(([n, i, k]) =>
  `<a href="#" data-to="${k}" data-tab ${n === active ? 'aria-current="page"' : ''}><span class="ind">${ic(i, n === active)}</span><span class="lbl">${n}</span></a>`).join('')}</nav>`;
const m3Icon = (i, label, extra = '') => `<button class="m3-ic" aria-label="${label}" ${extra}>${ic(i)}</button>`;
/* top app bars: small (arrow + left-aligned title) and medium (arrow row, headline below) */
const m3Small = title => `<header class="m3-bar small">${m3Icon('arrow-left', 'Back', 'data-back')}<h1>${title}</h1>${m3Icon('dots-three-vertical', 'More')}</header>`;
const m3Medium = title => `<header class="m3-bar medium"><div class="m3-row">${m3Icon('arrow-left', 'Back', 'data-back')}<span></span>${m3Icon('dots-three-vertical', 'More')}</div><h1>${title}</h1></header>`;
const m3Large = (title, sup) => `<header class="m3-bar large"><div class="m3-row"><span></span>${m3Icon('magnifying-glass', 'Search')}${m3Icon('dots-three-vertical', 'More')}</div><p class="sup">${sup}</p><h1>${title}</h1></header>`;
/* on Android, Needs you is an extended FAB on every tab (the iOS pill's job, in the platform's idiom) */
const m3Fab = () => `<button class="m3-fab" data-to="requests" aria-label="2 requests waiting. Open Requests."><span class="pulse" aria-hidden="true"></span><b>2 need you</b><span class="tm" data-left="q1" data-start="41" data-soon>0:41</span></button>`;

/* one Flutter codebase: the screen body is the same widget tree; the scaffold (bars, navigation, back, biometrics) is Material */
function android(html, bar) {
  const active = (html.match(/<a href="#" data-to="\w+" data-tab aria-current="page">[\s\S]*?<span>(\w+)<\/span>/) || [])[1];
  html = html.replace(chrome(), androidChrome())
    .replace(/<nav class="tabbar"[\s\S]*?<\/nav>/, active ? m3Nav(active) + m3Fab() : '')
    .replace(/<header class="lt">[\s\S]*?<\/header>/, m3Large('Home', 'Wednesday 25 November'))
    .replace(/<div class="navbar">[\s\S]*?<\/div>/, bar)
    .replaceAll(`${ic('scan')}Approve once`, `${ic('fingerprint')}Approve once`)
    .replace('Approve once asks for Face ID', 'Approve once asks for your fingerprint');
  return html;
}
const aHome = () => android(home(), '').replace('<button class="req-row" data-to="permission"', '<button class="req-row pressed" data-to="permission"');
const aPermission = () => android(permission(), m3Medium('Permission request'));
/* predictive back, mid-gesture: the page shrinks and follows the thumb, Home shows behind under a scrim, the edge arrow appears */
const aBack = () => `${androidChrome()}<div class="pb-under">${android(home(), '').replace(androidChrome(), '')}</div>
  <div class="pb-page">${aPermission().replace(androidChrome(), '')}</div>
  <span class="pb-arrow" aria-hidden="true">${ic('arrow-left')}</span>`;
Object.assign(SCREENS, {
  'a-home': aHome,
  'a-permission': aPermission,
  'a-back': aBack,
  'a-stop': () => android(session('stop'), m3Small('')),
});

/* ---------- S24 desktop: Computers › Pair a phone, and the Phones list (in the desktop app's own style) ---------- */
function desktop(el) {
  const rail = [['house', 'Overview'], ['terminal-window', 'Sessions'], ['gauge', 'Accounts'], ['desktop', 'Computers', 1], ['wrench', 'Setup']];
  el.innerHTML = `<div class="dk-title"><span class="dk-dots"><i></i><i></i><i></i></span><b>Harness Manager</b><span class="dk-search">${ic('magnifying-glass')}Filter accounts · Run a command <kbd>Ctrl K</kbd></span></div>
  <div class="dk-body">
    <nav class="dk-rail">${rail.map(([i, n, on]) => `<a ${on ? 'class="on"' : ''}>${ic(i)}${n}</a>`).join('')}</nav>
    <main class="dk-main">
      <header class="dk-h"><h1>Computers</h1><span class="sp"></span><button class="dk-btn">${ic('plus')}Invite a computer…</button><button class="dk-btn ac">${ic('device-mobile')}Pair a phone…</button></header>
      <section class="dk-sec"><h2>Computers <span>3</span></h2>
        <div class="dk-row"><b>DEVELOPMENTPC</b><span>this computer · 5 running</span><span class="ok">online</span></div>
        <div class="dk-row"><b>LAPTOP</b><span>through your hub · 2 running</span><span class="ok">online</span></div>
        <div class="dk-row"><b>MINI</b><span>last seen Mon 21:40</span><span class="dim">asleep</span></div></section>
      <section class="dk-sec"><h2>Phones <span>2</span></h2>
        <div class="dk-row phn"><b>${ic('device-mobile')}iPhone 17</b><span>Summaries, Transcripts, Control · last seen now · paired 9 Nov</span><button class="dk-btn sm">Grants…</button><button class="dk-btn sm neg">Revoke</button></div>
        <div class="dk-row phn"><b>${ic('device-mobile')}Pixel 9</b><span>Summaries · last seen 3 d ago · access expires in 11 d if it stays away</span><button class="dk-btn sm">Grants…</button><button class="dk-btn sm neg">Revoke</button></div>
        <p class="dk-note">A phone is never a full peer: it gets no recovery key and no admin token. Revoking confirms on this computer, the hub and the push relay.</p></section>
    </main>
  </div>
  ${el.dataset.desk === 'list' ? '' : `<div class="dk-scrim"></div>
  <section class="dk-dialog" role="dialog" aria-label="Pair a phone">
    <header><h2>Pair a phone</h2><span class="dk-x">${ic('x')}</span></header>
    <div class="dk-cols">
      <div class="dk-step done"><span class="n">1</span><h3>Scan or type the code</h3>
        <div class="dk-qr">${qr(25, 5)}<span class="used">${ic('check-circle', 1)}Code used</span></div>
        <p>On the phone: Harness Manager › Pair with a computer. Or type <b class="mono">481 920</b>.</p>
        <p class="dim">Open 2 min, 3 tries. Same network, or through your hub.</p></div>
      <div class="dk-step"><span class="n">2</span><h3>iPhone 17 wants access</h3>
        <p>Check the phone shows the same words:</p>
        <div class="dk-words"><span>amber</span><span>otter</span><span>nine</span></div>
        <label class="dk-ck on"><i>${ic('check')}</i><span><b>Summaries</b>Limits, sessions and usage</span></label>
        <label class="dk-ck on"><i>${ic('check')}</i><span><b>Transcripts</b>What each session said and did</span></label>
        <label class="dk-ck"><i></i><span><b>Control</b>Answer requests, reply and Stop. Off unless you tick it.</span></label>
        <div class="dk-act"><span class="dim">${ic('timer')}1:31 left</span><button class="dk-btn">Decline</button><button class="dk-btn ac">Approve</button></div></div>
    </div>
  </section>`}`;
}
(function desk() { document.querySelectorAll('[data-desk]').forEach(desktop); })();

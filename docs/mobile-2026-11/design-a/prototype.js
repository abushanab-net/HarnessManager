/* Calibre · clickable prototype: five journeys in one phone. Navigation itself is go() in calibre.js (push, back, tabs). */
(function prototype() {
  const ph = document.querySelector('.phone[data-proto]'); if (!ph) return;
  const J = [
    ['Pair a phone', 'welcome', [['welcome', 'Welcome: Pair with a computer'], ['scan', 'Scan: tap the frame to simulate a scan'], ['approve', 'Approve on the computer: it approves by itself after 3 s'], ['connected', 'Connected: Continue'], ['notify', 'Notifications: Allow'], ['home', 'Home']]],
    ['Answer a request', 'home', [['home', 'Home: tap the permission in Needs you'], ['permission', 'Permission: Approve once (Face ID)'], ['receipt', 'Receipt: the tick lands, then Back to Home']]],
    ['Use next', 'home', [['home', 'Home: tap the Use next card'], ['account', 'Account detail: the decision on the first screen']]],
    ['Stop a turn', 'sessions', [['sessions', 'Sessions: tap the first row'], ['session', 'Session detail: tap the Stop square'], ['stop', 'Stop sheet: Stop the turn'], ['stopped', 'Interrupt delivered']]],
    ['Progress and a badge', 'you', [['you', 'You: tap Progress'], ['progress', 'Progress: tap a badge'], ['badge', 'Badge detail sheet']]],
  ];
  const list = document.querySelector('[data-journeys]');
  list.innerHTML = J.map(([t, , steps], i) => `<section class="jr" data-j="${i}"><button class="jr-go" data-j="${i}"><b>${t}</b><span>${steps.length} steps</span></button>
    <ol>${steps.map(([k, s]) => `<li data-k="${k}">${s}</li>`).join('')}</ol></section>`).join('');
  let cur = 0;
  const mark = k => {
    const sec = list.querySelector(`[data-j="${cur}"].jr`); if (!sec) return;
    const lis = [...sec.querySelectorAll('li')], at = lis.findIndex(li => li.dataset.k === k);
    if (at >= 0) lis.forEach((li, i) => { li.classList.toggle('done', i < at); li.classList.toggle('now', i === at); });
  };
  const start = i => {
    cur = i; HIST.set(ph, []); clearTimeout(ph._auto); closeOverlays(ph);
    list.querySelectorAll('.jr').forEach(s => s.classList.toggle('on', +s.dataset.j === i));
    list.querySelectorAll('li').forEach(li => li.classList.remove('done', 'now'));
    ph.dataset.screen = J[i][1]; render(ph); ph.classList.remove('is-in'); requestAnimationFrame(() => requestAnimationFrame(() => ph.classList.add('is-in')));
    mark(J[i][1]);
  };
  list.addEventListener('click', e => { const b = e.target.closest('.jr-go'); if (b) start(+b.dataset.j); });
  document.addEventListener('proto:nav', e => mark(e.detail.to));
  // receipts and sheets are states inside a screen: watch for them to tick the last steps
  new MutationObserver(() => {
    if (ph.querySelector('.receipt.show')) mark('receipt');
    if (ph.querySelector('.sheet.in:not([hidden])')) mark('badge');
  }).observe(ph, { subtree: true, attributes: true, attributeFilter: ['class', 'hidden'] });
  document.querySelectorAll('[data-proto-theme]').forEach(b => b.addEventListener('click', () => {
    ph.dataset.theme = b.dataset.protoTheme;
    document.querySelectorAll('[data-proto-theme]').forEach(x => x.setAttribute('aria-pressed', x === b));
  }));
  document.addEventListener('calibre:mount', () => setTimeout(() => start(cur), 0));
})();

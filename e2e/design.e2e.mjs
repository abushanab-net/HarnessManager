// The design's measurements, as tests of their own: what the window must measure to be the 1.10 drawings
// (docs/project-review-2026-11/mockups: a.css, base.css and the four a-*.html pages). app.e2e.mjs imports this and
// runs it in the same window; the behaviour of each page stays in that file.
//
// The spec, in the Harness light theme at 1280 × 800:
//   the shell    a 208 px sidebar (the product name and the computer's name on one line under it, the items in order,
//                the version at the foot), a 44 px top bar on the canvas, every control at least 28 px tall
//   the type     --fs-title 20 (the page's name), --fs-section 15, --fs-body 13, --fs-small 12 (the floor), --fs-key 22
//   the head     every page: its stable name, one status sentence, its actions at the right
//   the badges   only for something to act on or watch, with its unit: "N live", "▲ N", "N updates"
// A page worker who redraws a page changes that page's numbers here, in `PARTS`, and nothing in the shell's.

/** Every main page: its address, its stable name, and (for a detail page) the name its own h1 carries. */
export const PAGES = [
  ['#home', 'Overview'], ['#sessions', 'Sessions'], ['#projects', 'Projects'], ['#accounts', 'Accounts'], ['#computers', 'Computers'],
  ['#mcp', 'MCP servers'], ['#skills', 'Skills'], ['#prompts', 'System prompts'], ['#routing', 'API keys & routing'], ['#apps', 'Apps'],
  ['#cli', 'CLI reference'], ['#settings', 'Settings'],
  ['#account?name=claude-main', 'Accounts', 'claude-main'], ['#session?profile=claude-main&id=s-main-1', 'Sessions', 'Longhorn backup target for staging'],
];
/** What the top bar's search says it does on each page. */
const FIND = { '#accounts': 'Filter accounts', '#projects': 'Filter projects', '#mcp': 'Filter MCP servers', '#skills': 'Filter skills' };
/** Text that may stay under 12 px, each with its reason (the list the 1.10 brief asks for). */
const SMALL_TEXT_ALLOWED = [
  'text.ring-val', // the percentage inside the 36 px week ring, an in-SVG label: 11 px ("100%" at 9.5 px), so it stays inside the 28 px inner circle
];
/** Each page's own parts, as its page worker drew them: [selector, computed property, value]. */
const PARTS = {
  '#home': [['.ov-deck', 'columnGap', '10px'], ['.ov-deck .card-head', 'height', '40px'], ['.ov-readouts b', 'fontSize', '15px'], ['.ov-table', 'tableLayout', 'fixed'], ['.ov-table td', 'height', '48px'], ['.ov-table th', 'fontSize', '12px']],
  '#accounts': [['.a-filters .seg > *', 'height', '24px'], ['.tool-group-header', 'height', '36px'], ['.acct-row', 'height', '56px'], ['.acct-row', 'columnGap', '14px'], ['.a-ring', 'width', '44px'], ['.a-ring b', 'fontSize', '12px'],
    ['.acct-row .lim', 'width', '72px'], ['.acct-row .acts .btn', 'height', '28px'], ['.acct-row .acts .btn.icon', 'width', '30px']],
  '#account?name=claude-main': [['.account-columns', 'display', 'grid'], ['.page-head .btn.primary', 'height', '30px']],
  // Sessions: one filter line; the groups in one card, a 36 px band a group, rows 44 px (b-sessions.html's middle zone)
  '#sessions': [['.a-fbar', 'display', 'flex'], ['.a-fbar .seg', 'height', '30px'], ['.a-lh', 'minHeight', '36px'], ['.a-gh', 'height', '36px'], ['.a-srow', 'minHeight', '44px'], ['.a-srow', 'padding', '0px 12px'], ['.a-gname', 'fontSize', '13px'], ['.a-title b', 'fontWeight', '600']],
  // Projects: one list in one card, a project a 52 px row
  '#projects': [['.a-plist', 'padding', '0px'], ['.a-prow.head', 'minHeight', '34px'], ['.a-pcard', 'minHeight', '52px'], ['.a-pcard', 'display', 'grid'], ['.a-pname', 'fontWeight', '650']],
  '#session?profile=claude-main&id=s-main-1': [['.a-conv', 'padding', '0px'], ['.a-chat', 'padding', '12px 14px'], ['.page-head .actions', 'flexWrap', 'nowrap'], ['.a-split .btn', 'height', '30px']],
  '#routing': [['.b-router-stats b', 'fontSize', '20px'], ['.b-router-stats', 'display', 'grid']],
  '#apps': [['.b-apps', 'tableLayout', 'fixed'], ['.b-apps td', 'verticalAlign', 'middle']],
  // MCP servers and Skills: ONE line above the matrix, which stays put with the tools' header under it
  '#mcp': [['.b-matrix', 'tableLayout', 'fixed'], ['.b-matrix td', 'padding', '8px 5px'], ['.b-matrix th', 'position', 'sticky'], ['.b-matrix th', 'top', '42px'], ['.b-one', 'position', 'sticky'], ['.b-one', 'minHeight', '42px']],
  '#skills': [['.b-one', 'display', 'flex'], ['.b-one', 'flexWrap', 'nowrap'], ['.b-one', 'position', 'sticky']],
  '#prompts': [['.b-prompt-grid', 'display', 'grid']],
  '#cli': [['.b-cli-grid', 'display', 'grid'], ['.b-cli-row', 'display', 'grid']],
  '#settings': [['.set', 'gridTemplateColumns', '200px 832px'], ['.set-find', 'height', '32px'], ['.set-nav a', 'height', '32px'], ['.set-pane', 'padding', '0px'], ['.pane-head', 'height', '48px'], ['.srow', 'minHeight', '60px'], ['.srow', 'gridTemplateColumns', '380px 402px'], ['.srow .what p', 'fontSize', '12px']],
  '#computers': [['.pc-head', 'height', '48px'], ['.kv dt', 'fontSize', '12px'],['.count .key', 'fontSize', '22px'], ['.ways', 'display', 'grid'], ['.way', 'minHeight', '68px'], ['.pane-head', 'height', '48px'], ['.srow', 'minHeight', '60px'], ['.sw > i', 'width', '36px']],
};

export async function designTests({ test, exec, nav, $, click, waitFor, assert, page, shot }) {
  /** Open a page and wait until it is the one drawn. */
  const open = async (hash) => {
    await nav(hash);
    await waitFor(() => exec("return import('./core.js').then((c) => c.S.drawn === location.hash)"), hash + ' drawn');
  };

  await test('design: the shell is the 1.10 drawing: a 208 px sidebar with its items in order, a 44 px top bar, controls at least 28 px tall', async () => {
    await open('#accounts');
    await $('[data-testid=accounts-table]');
    if (await exec("return document.getElementById('nav-setup').hidden")) await click('[data-testid=nav-setup]');
    await page.mouse.move(640, 600); // no hover colour on what is measured
    const d = await exec(`
      const cs = (e) => getComputedStyle(e), q = (s) => document.querySelector(s), box = (s) => q(s).getBoundingClientRect(), px = (n) => Math.round(n * 2) / 2;
      const rail = cs(q('aside.rail')), top = cs(q('.topbar')), on = q('.nav a.active'), item = q('[data-testid=nav-home]'), sub = q('[data-testid=nav-mcp]'), list = cs(q('#nav-setup'));
      const find = q('#find'), ver = q('[data-testid=nav-version]'), chip = cs(q('#live-strip'));
      return {
        frame: cs(q('.frame')).backgroundColor, railBg: rail.backgroundColor, topBg: top.backgroundColor, sheetBg: cs(q('.sheet')).backgroundColor,
        rail: [box('aside.rail').width, rail.paddingTop, rail.paddingRight, rail.paddingBottom, rail.paddingLeft, rail.borderRightWidth, rail.borderRightColor],
        top: [box('.topbar').height, top.paddingLeft, top.paddingRight, top.borderBottomWidth, top.borderBottomColor],
        brand: [cs(q('.brand-text b')).fontSize, cs(q('.brand-text b')).fontWeight, cs(q('#brand-sub')).fontSize, box('#brand-mark').width],
        brandSub: q('#brand-sub').textContent, brandLines: [px(box('.brand-text b').height), px(box('#brand-sub').height)],
        labels: [...document.querySelectorAll('aside.rail a[data-nav], aside.rail [data-testid=nav-setup]')].map((a) => (a.querySelector('.nav-label') || a).textContent.trim()),
        item: [px(item.getBoundingClientRect().height), cs(item).borderTopLeftRadius, cs(item).fontSize, cs(item).fontWeight, cs(item).columnGap, cs(item).paddingLeft, px(item.querySelector('.nav-icon').getBoundingClientRect().width)],
        active: [cs(on).backgroundColor, cs(on).color, on.querySelector('.nav-label').textContent.trim(), on.getAttribute('aria-current')],
        sub: [px(sub.getBoundingClientRect().height), cs(sub).fontSize, cs(sub).fontWeight], subList: [list.marginLeft, list.paddingLeft, list.borderLeftWidth],
        version: [cs(ver).fontSize, /^v\\d+\\.\\d+\\.\\d+/.test(ver.textContent), px(q('aside.rail').getBoundingClientRect().bottom - ver.getBoundingClientRect().bottom)],
        search: [px(box('.search-box').width), px(box('.search-box').height), cs(q('.search-box')).borderTopLeftRadius, find.placeholder, find.scrollWidth <= find.clientWidth, cs(q('.search-box .kbd')).fontSize],
        chip: [chip.height, chip.borderTopLeftRadius, chip.fontSize, chip.fontWeight], fresh: cs(q('#fresh')).fontSize,
        iconBtn: [...document.querySelectorAll('.topbar .icon-btn')].map((b) => [px(b.getBoundingClientRect().width), px(b.getBoundingClientRect().height), cs(b).borderTopLeftRadius].join()),
        // every control of the shell that is on screen, with a height under 28 px
        low: [...document.querySelectorAll('aside.rail a[href], aside.rail button, aside.rail [role=button], .topbar a[href], .topbar button, .topbar input')]
          .filter((e) => e.getBoundingClientRect().height > 0 && e.getBoundingClientRect().height < 28).map((e) => (e.dataset.testid || e.className) + ' ' + e.getBoundingClientRect().height),
        font: cs(document.body).fontFamily, inter: document.fonts.check('13px "Inter"'), mark: !!q('#brand-mark svg linearGradient'),
      };`);
    assert.equal(d.frame, 'rgb(242, 246, 248)', 'the canvas');
    assert.equal(d.railBg, 'rgb(233, 239, 242)', 'the sidebar');
    assert.equal(d.topBg, 'rgba(0, 0, 0, 0)', 'the top bar sits on the canvas');
    assert.equal(d.sheetBg, 'rgba(0, 0, 0, 0)', 'an open page: cards sit on the canvas');
    assert.deepEqual(d.rail, [208, '10px', '8px', '8px', '8px', '1px', 'rgb(223, 233, 237)'], 'the sidebar: 208 px, padded 10 / 8 / 8, a faint line at its right');
    assert.deepEqual(d.top, [44, '16px', '16px', '1px', 'rgb(223, 233, 237)'], 'the top bar: 44 px, padded 16, a faint line under it');
    assert.deepEqual(d.brand, ['14px', '700', '12px', 26], 'the product name 14 / 700, the computer under it at 12, the mark 26');
    const computer = await exec("return import('./core.js').then((c) => c.S.state.computer)");
    assert.equal(d.brandSub, computer, 'under the product name: the computer\'s name, and no account count');
    assert.ok(d.brandLines[0] <= 20 && d.brandLines[1] <= 17, 'each on one line: ' + d.brandLines);
    assert.deepEqual(d.labels, ['Overview', 'Sessions', 'Projects', 'Accounts', 'Computers', 'Setup', 'MCP servers', 'Skills', 'System prompts', 'API keys & routing', 'Apps', 'CLI reference', 'Settings'], 'the items, in the drawing\'s order');
    assert.deepEqual(d.item, [34, '7px', '13px', '600', '10px', '10px', 16], 'an item: 34 px, radius 7, 13 / 600, a 16 px icon 10 px from its label');
    assert.deepEqual(d.active, ['rgba(8, 101, 108, 0.1)', 'rgb(8, 101, 108)', 'Accounts', 'page'], 'the lit item: shaded with the accent, and named to a screen reader');
    assert.deepEqual(d.sub, [28, '13px', '500'], 'a Setup page: 28 px, 13 / 500');
    assert.deepEqual(d.subList, ['17px', '9px', '1px'], 'the Setup pages hang from a line under Setup');
    assert.deepEqual(d.version.slice(0, 2), ['12px', true], 'the version, in 12 px');
    assert.ok(d.version[2] <= 12, 'at the foot of the sidebar: ' + d.version[2]);
    assert.deepEqual(d.search.slice(0, 3), [340, 30, '7px'], 'the search box: 340 × 30, radius 7');
    assert.equal(d.search[3], 'Filter accounts', 'it says what it does on this page');
    assert.ok(d.search[4], 'and its words are never cut off');
    assert.equal(d.search[5], '12px', 'its key hint (/) in 12 px');
    assert.deepEqual(d.chip, ['28px', '99px', '12px', '600'], 'a chip: 28 px, round, 12 / 600');
    assert.equal(d.fresh, '12px', 'limits read … ago, in 12 px');
    assert.deepEqual(d.iconBtn, ['30,30,7px', '30,30,7px'], 'the tour and refresh buttons: 30 × 30, radius 7');
    assert.deepEqual(d.low, [], 'every control of the shell is at least 28 px tall');
    assert.ok(d.font.includes('Inter') && d.inter, 'Inter loaded');
    assert.ok(d.mark, 'the Interlocking H in the sidebar');
  });

  await test('design: the type scale, the page head and the shared parts measure as the drawings set them', async () => {
    await open('#accounts');
    await $('[data-testid=accounts-table]');
    await page.mouse.move(640, 600);
    const d = await exec(`
      const cs = (e) => getComputedStyle(e), q = (s) => document.querySelector(s), root = cs(document.documentElement);
      const body = cs(q('.page-body')), head = q('.page-head'), h1 = cs(q('.page-head h1.title')), st = cs(q('.page-head .status')), btn = q('.page-head .btn:not(.primary)'), pri = q('.page-head .btn.primary');
      return {
        tokens: ['--fs-title', '--fs-section', '--fs-body', '--fs-small', '--fs-key'].map((t) => root.getPropertyValue(t).trim()),
        body: [cs(document.body).fontSize, cs(document.body).fontFamily.split(',')[0].replace(/"/g, '')],
        pad: [body.paddingTop, body.paddingRight, body.paddingBottom, body.paddingLeft, body.rowGap],
        head: [cs(head).minHeight, cs(head).alignItems, head === q('.page-body').firstElementChild],
        h1: [h1.fontSize, h1.fontWeight, h1.fontFamily.split(',')[0].replace(/"/g, ''), h1.lineHeight], status: [st.fontSize, st.color, st.lineHeight, st.whiteSpace],
        right: Math.abs(q('.page-head .actions').getBoundingClientRect().right - (q('.page-body').getBoundingClientRect().right - parseFloat(body.paddingRight))),
        btn: [Math.round(btn.getBoundingClientRect().height), cs(btn).borderTopLeftRadius, cs(btn).fontSize, cs(btn).fontWeight], primary: [cs(pri).backgroundColor, document.querySelectorAll('.page-head .btn.primary').length],
        panel: [cs(q('.panel-card')).borderTopLeftRadius, cs(q('.panel-card')).backgroundColor],
      };`);
    assert.deepEqual(d.tokens, ['20px', '15px', '13px', '12px', '22px'], 'the scale: title 20, section 15, body 13, small 12 (the floor), key 22');
    assert.deepEqual(d.body, ['13px', 'Inter'], 'body 13 px Inter');
    assert.deepEqual(d.pad, ['14px', '14px', '14px', '14px', '12px'], 'the page: 14 px around, its parts 12 apart');
    assert.deepEqual(d.head, ['44px', 'center', true], 'the head comes first, 44 px at least');
    assert.deepEqual(d.h1, ['20px', '700', 'Inter', '24px'], 'the page\'s name: Inter 20 / 700');
    assert.deepEqual(d.status, ['13px', 'rgb(34, 56, 66)', '18px', 'nowrap'], 'the status sentence: 13 px, one line');
    assert.ok(d.right < 1, 'the actions sit at the right edge: ' + d.right);
    assert.deepEqual(d.btn, [30, '7px', '13px', '600'], 'a button: 30 px, radius 7, 13 / 600');
    assert.deepEqual(d.primary, ['rgb(8, 101, 108)', 1], 'one primary button, in the accent');
    assert.deepEqual(d.panel, ['10px', 'rgb(255, 255, 255)'], 'panel cards: radius 10, white');
    // Add account is a dialog over the list
    await click('[data-testid=add-account]');
    const dlg = await exec("const m = document.querySelector('.modal-card'), s = getComputedStyle(m); return { radius: s.borderTopLeftRadius, width: m.getBoundingClientRect().width, kind: getComputedStyle(document.querySelector('.kind-pick-card.active')).borderColor, tool: getComputedStyle(document.querySelector('.tool-pick-card.active')).borderColor };");
    assert.equal(dlg.radius, '12px', 'a dialog: radius 12');
    assert.equal(dlg.width, 640, 'the dialog is 640 wide');
    assert.equal(dlg.kind, 'rgb(8, 101, 108)', 'the chosen kind in the accent');
    assert.equal(dlg.tool, 'rgb(8, 101, 108)', 'the chosen tool in the accent');
    await exec("document.querySelector('[data-act=dialog-close]').click()");
    // an account's page: the readout tiles, the tabs, the avatar
    await open('#account?name=claude-main');
    await $('.account-stat');
    const a = await exec(`
      const cs = (el) => getComputedStyle(el), q = (s) => document.querySelector(s), box = (s) => q(s).getBoundingClientRect();
      const panel = cs(q('.panel-card')), pt = cs(q('.panel-title')), stat = cs(q('.account-stat')), sb = cs(q('.account-stat > b'));
      return { panel: [panel.borderTopLeftRadius, panel.paddingTop, panel.paddingLeft, panel.rowGap], title: [pt.fontSize, pt.fontWeight],
        stat: [stat.borderTopLeftRadius, stat.paddingTop, stat.paddingLeft], statB: [sb.fontWeight, sb.fontSize], statGap: cs(q('.account-stats')).columnGap,
        tab: cs(q('.page-tabs .filter-chip.active')).color, avatar: [box('.account-avatar').width, cs(q('.account-avatar')).borderTopLeftRadius], back: q('.back-chip').textContent.trim() };`);
    assert.deepEqual(a.panel, ['10px', '11px', '13px', '9px'], 'panel cards: radius 10, padding 11 / 13, parts 9 apart');
    assert.deepEqual(a.title, ['12.5px', '700'], 'section titles 12.5 / 700');
    assert.deepEqual(a.stat, ['10px', '12px', '12px'], 'readout tiles: radius 10, padding 12');
    assert.deepEqual(a.statB, ['700', '18px'], 'readout values 18 / 700 at 1280');
    assert.equal(a.statGap, '10px', '10 px between tiles');
    assert.equal(a.tab, 'rgb(8, 101, 108)', 'the current tab in the accent');
    assert.deepEqual(a.avatar, [44, '10px'], 'the avatar 44, radius 10');
    assert.equal(a.back, '← Accounts', 'a detail page keeps its back line');
  });

  await test('design: every page shows its stable name and one status sentence, names the window, and takes the focus', async () => {
    for (const [hash, name, own] of PAGES) {
      await exec("document.getElementById('find').blur()");
      await open(hash);
      const d = await exec(`const q = (s) => document.querySelector(s), h1 = q('#sheet .page-head h1.title'), st = q('#sheet [data-testid=page-status]'), on = q('aside.rail a.active .nav-label, aside.rail a.nav-subitem.active');
        return { h1s: document.querySelectorAll('#sheet h1').length, title: h1 && h1.textContent.trim(), status: st && st.textContent.trim(), lines: st && Math.round(st.getBoundingClientRect().height / parseFloat(getComputedStyle(st).lineHeight)),
          doc: document.title, focus: document.activeElement && document.activeElement.id, lit: on && on.textContent.trim(), find: q('#find').placeholder, back: !!q('#sheet .back-chip'),
          first: q('#sheet .page-body').firstElementChild.className, pad: getComputedStyle(q('#sheet > .page-body')).padding, gap: getComputedStyle(q('#sheet > .page-body')).rowGap };`);
      assert.equal(d.h1s, 1, `${hash}: one h1`);
      assert.equal(d.doc, `${name} · Harness Manager`, `${hash}: the window's title`);
      assert.equal(d.lit, name, `${hash}: its item is lit in the sidebar`);
      assert.equal(d.focus, 'sheet', `${hash}: the page has the keyboard focus`);
      assert.equal(d.find, FIND[hash] || 'Search sessions', `${hash}: the search box says what it does here`);
      assert.deepEqual([d.pad, d.gap], ['14px', '12px'], `${hash}: the page padded 14, its parts 12 apart`);
      assert.ok(d.status && d.status.length > 8, `${hash}: a status under the name: ${d.status}`);
      if (own) {
        // a detail page: the back line, then the item's own name
        assert.ok(d.back && d.title.includes(own), `${hash}: its own name after a back line: ${d.title}`);
      } else {
        assert.equal(d.title, name, `${hash}: its stable name`);
        assert.ok(d.first.includes('page-head'), `${hash}: the head comes first`);
        assert.ok(/[.…]$/.test(d.status), `${hash}: the status is a sentence: ${d.status}`);
        assert.equal(d.lines, 1, `${hash}: one line of status`);
      }
      for (const [sel, prop, v] of PARTS[hash] || []) {
        const got = await exec(`const e = document.querySelector(${JSON.stringify(sel)}); return e ? getComputedStyle(e)[${JSON.stringify(prop)}] : 'missing';`);
        assert.equal(got, v, `${hash}: ${sel} ${prop} as drawn`);
      }
      const cards = await exec("const c = [...document.querySelectorAll('#sheet .panel-card, #sheet .a-grp')]; return [c.length, [...new Set(c.map((x) => getComputedStyle(x).borderTopLeftRadius))].join()]");
      assert.ok(cards[0] > 0 && cards[1] === '10px', `${hash}: drawn in cards with 10 px corners: ${cards}`);
      // every main page in the light theme, for review (app.e2e.mjs takes the dark ones as 17-… and 18-dark-…)
      await shot('20-light-' + (own ? name.toLowerCase() + '-detail' : name.toLowerCase().replace(/[^a-z]+/g, '-')));
    }
  });

  await test('design: no text a person reads is under 12 px on any main page', async () => {
    /** Every element on screen that holds text of its own, with its computed size: those under 12 px, and how many were read. */
    const scan = () => exec(`const allowed = arguments[0], out = []; let read = 0;
        for (const el of document.querySelectorAll('.frame *')) {
          const r = el.getBoundingClientRect();
          if (!r.width || !r.height || ![...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim())) continue;
          const fs = parseFloat(getComputedStyle(el).fontSize);
          read++;
          if (fs < 12 && !allowed.some((a) => el.matches(a))) out.push(fs + 'px ' + el.tagName.toLowerCase() + '.' + String(el.className.baseVal ?? el.className).trim().replace(/\\s+/g, '.') + ' "' + el.textContent.trim().slice(0, 30) + '"');
        }
        return [[...new Set(out)], read];`, [SMALL_TEXT_ALLOWED]);
    // the measure itself, first: a line of 11 px text put on the page is found
    await open('#home');
    await exec("document.querySelector('#sheet .page-body').insertAdjacentHTML('beforeend', '<span id=\"probe-11\" style=\"font-size:11px\">eleven</span>')");
    assert.deepEqual((await scan())[0], ['11px span. "eleven"'], 'the measure finds small text');
    await exec("document.getElementById('probe-11').remove()");
    for (const [hash] of PAGES) {
      await open(hash);
      const [small, read] = await scan();
      assert.ok(read > 30, `${hash}: the page's text was measured (${read} elements)`);
      assert.deepEqual(small, [], `${hash}: text under 12 px`);
    }
  });

  await test('design: a sidebar badge appears only for something to act on or watch, with its unit', async () => {
    await open('#home');
    const r = await exec(`return import('./core.js').then(async (c) => {
      const q = (id) => { const e = document.getElementById(id); return e.hidden ? null : e.textContent.trim(); }, out = {};
      const keep = c.S.state, soon = Date.now() + 3600e3, p = keep.profiles.find((x) => x.harness === 'claude');
      const acct = (o) => ({ ...p, running: [], kind: 'subscription', identity: { ...p.identity, logged_in: true, needs_login: false }, quota: null, ...o });
      // nothing running, nothing to act on: no badge at all
      c.S.state = { ...keep, profiles: [acct({})] }; c.redrawLive(); c.redrawAttention(); c.setAppUpdates(0);
      out.none = [q('n-live'), q('n-attention'), q('apps-count'), document.getElementById('live-strip').hidden];
      // two sessions running; a login expired, one never made, a limit used up; an API key and a limit already reset do not count
      c.S.state = { ...keep, profiles: [acct({ running: [{ pid: 1 }, { pid: 2 }] }), acct({ identity: { ...p.identity, logged_in: true, needs_login: true } }), acct({ identity: { ...p.identity, logged_in: false } }),
        acct({ quota: { tiers: [{ id: 'week', label: 'Week', used: 100, resets_at: soon }] } }), acct({ kind: 'api_key', identity: { ...p.identity, logged_in: false } }),
        acct({ quota: { tiers: [{ id: 'week', label: 'Week', used: 100, resets_at: Date.now() - 1000 }, { id: 'session', label: '5-hour', used: 99.4, resets_at: soon }] } })] };
      c.redrawLive(); c.redrawAttention(); c.setAppUpdates(1);
      out.some = [q('n-live'), q('n-attention'), q('apps-count'), document.querySelector('#live-strip .live-n').textContent];
      out.titles = [document.getElementById('n-attention').title, !!document.querySelector('#n-attention svg')];
      c.setAppUpdates(3); out.updates = q('apps-count');
      c.S.state = keep;
      await c.loadState(); c.setAppUpdates(keep.app_updates || 0);
      // what the sidebar reads besides: the items' names, and nothing else
      out.rest = [...document.querySelectorAll('aside.rail a[data-nav]')].map((a) => [...a.childNodes].filter((n) => !(n.nodeType === 1 && n.classList.contains('badge'))).map((n) => n.textContent).join('').trim());
      out.gone = ['today-cost', 'n-projects', 'n-accounts', 'n-mcp', 'n-skills', 'n-apps'].filter((id) => document.getElementById(id));
      return out; })`);
    assert.deepEqual(r.none, [null, null, null, true], 'nothing to act on or watch: no badge, no live chip');
    assert.deepEqual(r.some, ['2 live', '3', '1 update', '2 live'], 'Sessions "2 live", Accounts 3 that need attention, Setup "1 update"');
    assert.deepEqual(r.titles, ['3 accounts need attention', true], 'the Accounts badge: the ▲ and, on hover, what the number counts');
    assert.equal(r.updates, '3 updates');
    assert.deepEqual(r.rest, ['Overview', 'Sessions', 'Projects', 'Accounts', 'Computers', 'MCP servers', 'Skills', 'System prompts', 'API keys & routing', 'Apps', 'CLI reference', 'Settings'], 'no cost beside Overview, no bare count beside a page');
    assert.deepEqual(r.gone, [], 'the old counts are gone');
  });
}

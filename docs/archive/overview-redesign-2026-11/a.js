// A, Blueprint (KERNL's drafted map and attention rail, Pulser's shared-hairline cells).
(function () {
  const { NOW, MEASURED, H, DAY, computers, accounts, tools, providers, router, edges, running, kpi, series, days, replies, fmt, money, dur, at, clock, esc, pct, label, pcName, pcCol, toolLabel, burn, shell } = HM;

  // a measured series against yesterday's, drawn like a plot: hour ticks on a baseline, yesterday dashed
  function plot(vals, prev) {
    const n = 24, all = vals.concat(prev), max = Math.max(...all) * 1.08, X = (i) => (i / (n - 1)) * 200, Y = (v) => 40 - (v / max) * 34;
    const line = (a) => a.map((v, i) => (i ? 'L' : 'M') + X(i).toFixed(1) + ' ' + Y(v).toFixed(1)).join(' ');
    const ticks = Array.from({ length: n }, (_, i) => `<line x1="${X(i)}" x2="${X(i)}" y1="41" y2="${i % 6 ? 43 : 45}" stroke="var(--m)" stroke-width="1" vector-effect="non-scaling-stroke"/>`).join('');
    const last = vals.length - 1;
    return `<svg class="plot" viewBox="0 0 200 46" preserveAspectRatio="none" aria-hidden="true">
      <line x1="0" x2="200" y1="41" y2="41" stroke="var(--line)" vector-effect="non-scaling-stroke"/>${ticks}
      <path d="${line(prev)}" fill="none" stroke="var(--m)" stroke-dasharray="2 3" stroke-width="1" vector-effect="non-scaling-stroke" opacity=".7"/>
      <path d="${line(vals)}" fill="none" stroke="var(--ac)" stroke-width="1.6" vector-effect="non-scaling-stroke"/>
      <line x1="${X(last)}" x2="${X(last)}" y1="2" y2="41" stroke="var(--ac)" stroke-width="1" stroke-dasharray="1 2" vector-effect="non-scaling-stroke"/></svg>`;
  }

  function kpis() {
    const t = kpi.tokens, c = kpi.cost, hit = kpi.hit;
    const prevOf = (a, f) => a.map((v, i) => v * (0.82 + 0.1 * Math.sin(i * f)));
    const split = Object.entries(t.by).map(([id, v]) => `<i style="flex:${v};background:${pcCol(id)}" title="${pcName(id)} ${fmt(v)}"></i>`).join('');
    const cw = burn().find((b) => b.out);
    return `<section class="blk kpis" aria-label="Key numbers">
      <div class="cell kpi click on"><div class="lab"><span>Tokens today</span><b>${pct(t.v, t.prev)}</b></div><strong>${fmt(t.v)}</strong>${plot(series.tokens, prevOf(series.tokens, 0.7))}
        <div class="split">${split}</div><div class="foot">Studio ${fmt(t.by.studio)}, Lab ${fmt(t.by.lab)}, Surface ${fmt(t.by.surface)}</div></div>
      <div class="cell kpi click"><div class="lab"><span>Cost at list prices</span><b class="warn">+${money(c.v - c.prev)}</b></div><strong>${money(c.v)}</strong>${plot(series.cost, prevOf(series.cost, 0.4))}
        <div class="foot">Billed per token: ${money(c.billed)}. Subscriptions are not.</div></div>
      <div class="cell kpi"><div class="lab"><span>Read from cache</span><b>+${Math.round((hit.v - hit.prev) * 100)} pt</b></div><strong>${Math.round(hit.v * 100)}<small>%</small></strong>${plot(series.hit.map((x) => x * 1e3), series.hit.map((x) => (x - 0.03) * 1e3))}
        <div class="foot">${kpi.requests.v.toLocaleString()} requests, ${pct(kpi.requests.v, kpi.requests.prev)} on yesterday</div></div>
      <div class="cell kpi risk"><div class="lab"><span>Limit risk</span><b class="warn">estimate</b></div><strong>1<small> account</small></strong>
        <div class="dim" style="height:2.75rem;background:none"><div class="ln" style="left:0;width:100%;top:20px"></div><div class="short" style="left:${(cw.out / cw.reset) * 100}%;width:${100 - (cw.out / cw.reset) * 100}%"></div>
        <span class="lbl" style="left:${(cw.out / cw.reset) * 50}%;top:2px">≈ ${dur(cw.out)} left</span><span class="lbl" style="left:${50 + (cw.out / cw.reset) * 50}%;top:24px;color:var(--warn)">${dur(cw.reset - cw.out)} short</span></div>
        <div class="foot"><b>${esc(cw.name)}</b> ${cw.win.toLowerCase()} runs out before its reset</div></div>
    </section>`;
  }

  // ---------- the schematic ----------
  const ROWS = {
    'claude-max': 0, 'claude-team': 1, 'codex-pro': 2, 'opencode-routed': 3, 'pi-openrouter': 4, antigravity: 5,
    claude: 0.5, codex: 2, opencode: 3, pi: 4, agy: 5, router: 3.5,
    anthropic: 0.5, openai: 1.85, deepseek: 2.85, openrouter: 3.75, groq: 4.65, google: 5.6,
  };
  function schematic(W) {
    const nw = Math.max(128, Math.min(172, W * 0.18)), nh = 46, step = 58, top = 30;
    const colX = [0, W * 0.27, W * 0.53, W - nw];
    const nodes = [];
    const tone = (a) => (a.needsLogin ? 'neg' : a.windows && a.windows.some((w) => w.out) ? 'warn' : 'ok');
    for (const a of accounts) {
      const w = a.windows && a.windows[a.windows.length - 1];
      nodes.push({ id: a.id, col: 0, t: a.label, s: a.needsLogin ? 'login expired' : a.cap ? `${money(a.cap.spent)} of $${a.cap.cap} today` : a.kind === 'routed' ? 'API key, routed' : `${100 - w.used}% of week left`, tone: tone(a), hot: a.tps > 0 });
    }
    for (const t of tools) nodes.push({ id: t.id, col: 1, t: t.label, s: `${t.ver}${running.filter((r) => r.tool === t.label).length ? ', ' + running.filter((r) => r.tool === t.label).length + ' open' : ''}`, hot: edges.some((e) => e[1] === t.id && e[3] > 0) });
    nodes.push({ id: 'router', col: 2, t: 'Local router', s: `:${router.port}, ${router.routes} routes, ${router.errors} errors`, hot: true, router: true });
    for (const p of providers) nodes.push({ id: p.id, col: 3, t: p.label, s: p.paused ? `paused, retry ${dur(p.retry)}` : p.tps ? `${fmt(p.tps)} tok/s, ${p.first} s first` : 'no traffic', tone: p.paused ? 'neg' : p.tps ? 'ok' : '', hot: p.tps > 0 });
    nodes.forEach((n) => { n.x = colX[n.col]; n.y = top + ROWS[n.id] * step; n.w = n.router ? nw * 0.92 : nw; n.h = n.router ? nh + 8 : nh; if (n.router) n.y -= 4; });
    const by = Object.fromEntries(nodes.map((n) => [n.id, n]));
    const Ht = top + 6.6 * step;
    const r = 7;
    const paths = edges.map(([a, b, kind, rate], i) => {
      const A = by[a], B = by[b], x1 = A.x + A.w, y1 = A.y + A.h / 2, x2 = B.x, y2 = B.y + B.h / 2;
      const into = edges.filter((e) => e[1] === b).indexOf(edges[i]);
      const mx = x1 + (x2 - x1) * (0.42 + 0.12 * into);
      let d;
      if (Math.abs(y2 - y1) < 1) d = `M${x1} ${y1} H${x2}`;
      else { const s = y2 > y1 ? 1 : -1, rr = Math.min(r, Math.abs(y2 - y1) / 2); d = `M${x1} ${y1} H${mx - rr} Q${mx} ${y1} ${mx} ${y1 + s * rr} V${y2 - s * rr} Q${mx} ${y2} ${mx + rr} ${y2} H${x2}`; }
      return { a, b, kind, rate, d };
    });
    const tick = (n) => `<path class="tick" d="M${n.x - 4} ${n.y} h8 M${n.x} ${n.y - 4} v8 M${n.x + n.w + 4} ${n.y + n.h} h-8 M${n.x + n.w} ${n.y + n.h + 4} v-8"/>`;
    const node = (n) => `<g class="n ${n.hot ? '' : 'idle'}" data-node="${n.id}" tabindex="0" role="button" aria-label="${esc(n.t + ', ' + n.s)}">
      <rect class="box" x="${n.x}" y="${n.y}" width="${n.w}" height="${n.h}" rx="2"/>${n.hot ? `<rect class="hatch" x="${n.x + 1}" y="${n.y + 1}" width="7" height="${n.h - 2}"/>` : ''}
      <text class="t" x="${n.x + 16}" y="${n.y + 19}">${esc(n.t)}</text><text class="s ${n.tone || ''}" x="${n.x + 16}" y="${n.y + 35}">${esc(n.s)}</text>${n.router ? tick(n) : ''}</g>`;
    const heads = ['Accounts', 'Tools', 'Router', 'Providers'].map((t, i) => `<text class="colh" x="${colX[i]}" y="12">${t}</text><line x1="${colX[i]}" x2="${colX[i] + (i === 3 ? nw : 24)}" y1="18" y2="18" stroke="var(--line)"/>`).join('');
    const svg = `<svg viewBox="-8 0 ${W + 16} ${Ht}" height="${Ht}" role="img" aria-label="Live routes: accounts to tools to the router to providers">
      <defs><pattern id="hatchA" width="4" height="4" patternUnits="userSpaceOnUse" patternTransform="rotate(45)"><line x1="0" y1="0" x2="0" y2="4" stroke="var(--ac)" stroke-width="1.4"/></pattern></defs>
      ${heads}<g class="edges">${paths.map((p) => `<path class="e ${p.rate || p.kind === 'r' ? p.kind : 'idle'}" data-a="${p.a}" data-b="${p.b}" d="${p.d}"/>`).join('')}</g>
      <g class="beads"></g><g class="nodes">${nodes.map(node).join('')}</g><g class="callouts"></g></svg>`;
    return { svg, paths, by, Ht };
  }
  function callout(svgEl, path, frac, dx, dy, lines, cls = '') {
    const len = path.getTotalLength(), p = path.getPointAtLength(len * frac);
    const bw = Math.max(...lines.map((l) => l.length)) * 6.6 + 16, bh = lines.length * 14 + 10, bx = p.x + dx, by = p.y + dy;
    const ex = dx > 0 ? bx : bx + bw, ey = by + bh / 2;
    const g = document.createElementNS('http://www.w3.org/2000/svg', 'g');
    g.setAttribute('class', 'callout ' + cls);
    g.innerHTML = `<circle cx="${p.x}" cy="${p.y}" r="2.4"/><path d="M${p.x} ${p.y} L${p.x + dx * 0.45} ${ey} L${ex} ${ey}"/><rect x="${bx}" y="${by}" width="${bw}" height="${bh}" rx="1.5"/>
      ${lines.map((l, i) => `<text class="${i ? '' : 'k'}" x="${bx + 8}" y="${by + 16 + i * 14}">${esc(l)}</text>`).join('')}`;
    svgEl.querySelector('.callouts').append(g);
  }

  // ---------- limits: what is measured (used, reset) against what is estimated (run-out) ----------
  function limits() {
    const L = burn(), span = 7 * DAY, P = (ms) => Math.min(100, (ms / span) * 100);
    const axis = Array.from({ length: 7 }, (_, i) => new Date(NOW + i * DAY).toLocaleDateString('en-GB', { weekday: 'short' }) + (i ? '' : ' now'));
    const rows = L.map((b) => {
      const tone = b.left < 20 ? 'neg' : b.left < 45 ? 'warn' : '';
      const w = P(b.reset), x = b.out ? P(b.out) : null;
      return `<div class="limrow"><div class="nm"><b>${esc(b.name)}</b><small>${esc(b.win)}</small></div>
        <div><div class="gauge" title="${Math.round(b.used)}% used"><i class="${tone}" style="width:${b.used}%"></i>${b.at ? `<u style="left:${Math.min(99.5, b.at)}%" title="≈ ${b.at}% at the reset at this pace"></u>` : ''}</div><small>${Math.round(b.used)}% now${b.at ? ', ≈' + Math.min(100, b.at) + '% at reset' : ''}</small></div>
        <div class="dim"><div class="ln" style="left:0;width:${w}%"></div><span class="lbl" style="left:${Math.max(6, Math.min(w / 2, 92))}%">resets ${at(b.reset)}</span>
          ${x != null ? `<div class="short" style="left:${x}%;width:${w - x}%"></div><span class="x" style="left:${x}%"></span><span class="xl" style="left:${x}%">≈ runs out ${at(b.out)}, ${dur(b.reset - b.out)} short</span>` : ''}</div></div>`;
    }).join('');
    const unread = accounts.filter((a) => a.needsLogin).map((a) => `<div class="limrow"><div class="nm"><b>${esc(a.label)}</b><small>not read</small></div><div><small class="est">—</small></div><div class="dim"><span class="lbl" style="left:12%;transform:none;color:var(--neg)">Sign in to read its limits</span></div></div>`).join('');
    return `<section class="blk lims"><div class="cell"><div class="ch"><h2>Limits</h2><span class="note">bars measured ${Math.round((NOW - MEASURED) / 1e3)} s ago · dashed marks are estimates at this pace</span></div>
      <div class="limhd"><span></span><span class="est">used</span><div class="axis">${axis.map((d) => `<span>${d}</span>`).join('')}</div></div>${rows}${unread}</div></section>`;
  }

  function heat(W) {
    const shade = ['var(--h0)', 'var(--h1)', 'var(--h2)', 'var(--h3)', 'var(--h4)'];
    const nz = days.map((d) => d.tokens).filter(Boolean).sort((a, b) => a - b), q = (p) => nz[Math.floor(p * (nz.length - 1))];
    const cuts = [q(0.25), q(0.5), q(0.75)], top = Math.max(...nz);
    const lv = (v) => (!v ? 0 : v >= top ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4);
    const off = (days[0].day.getDay() + 6) % 7, cols = Math.ceil((days.length + off) / 7), left = 28, gap = 2;
    const cell = Math.max(6, Math.floor((W - left) / cols) - gap), Hh = 16 + 7 * (cell + gap);
    let s = `<svg viewBox="0 0 ${left + cols * (cell + gap)} ${Hh}" role="img" aria-label="Daily tokens over 53 weeks">`;
    let lastM = -1;
    days.forEach((d, i) => {
      const c = Math.floor((i + off) / 7), r = (i + off) % 7, x = left + c * (cell + gap), y = 14 + r * (cell + gap);
      if (r === 0 && d.day.getMonth() !== lastM) { s += `<text x="${x}" y="9" font-size="10" fill="var(--m)" font-family="var(--mono)">${d.day.toLocaleDateString('en-GB', { month: 'short' })}</text>`; lastM = d.day.getMonth(); }
      s += `<rect class="d" x="${x}" y="${y}" width="${cell}" height="${cell}" fill="${shade[lv(d.tokens)]}"${d.tokens >= top ? ' stroke="var(--ink)" stroke-width="1.5"' : ''}><title>${d.day.toDateString()}: ${fmt(d.tokens)} tokens</title></rect>`;
    });
    [['Mon', 0], ['Wed', 2], ['Fri', 4]].forEach(([t, r]) => { s += `<text x="0" y="${14 + r * (cell + gap) + cell - 1}" font-size="10" fill="var(--m)" font-family="var(--mono)">${t}</text>`; });
    return s + '</svg>';
  }

  function render(app, o) {
    const cw = burn().find((b) => b.out);
    const resets = burn().filter((b) => b.reset < 1.2 * DAY).sort((a, b) => a.reset - b.reset).slice(0, 5);
    const main = `
      <div class="hd"><div>
        <div class="status"><i class="live"></i>Live on 3 computers. ${running.length} sessions open, router on :${router.port}.</div>
        <h1>Overview</h1>
        <p class="finding">Claude Max carried 44% of today's tokens. <b>${esc(cw.name)}</b> will run out of its ${cw.win.toLowerCase()} limit <span class="est">≈</span> ${at(cw.out)}, ${dur(cw.reset - cw.out)} before it resets.</p></div>
        <div class="acts"><span class="seg" role="group" aria-label="Tool"><button class="on">All</button><button>Claude Code</button><button>Codex</button><button>OpenCode</button><button>Pi</button></span>
          <span class="seg" role="group" aria-label="Range"><button class="on">Today</button><button>7d</button><button>30d</button></span><button class="btn">${HM.icon('play', 13)} Start Claude Team</button></div></div>
      ${kpis()}
      <section class="blk main2">
        <div class="cell schem"><div class="ch"><h2>Live routes</h2><span class="note">${fmt(accounts.reduce((a, x) => a + x.tps, 0))} tok/s now · last 5 min, read each minute</span><span class="r note">hover a box to trace it</span></div><div class="schhost"></div>
          <div class="legend"><span><i class="c"></i>Signed-in plan, straight to the provider</span><span><i class="a"></i>API key, through the local router</span><span><i class="r"></i>Paused, or a login to renew</span></div></div>
        <div class="rail">
          <div><div class="ch"><h2>Needs attention</h2><span class="r big">3</span></div><div class="att">
            <div class="it"><i class="sv"></i><b>${esc(cw.name)}, weekly</b><span class="est">≈ ${at(cw.out)}</span><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace. Move work to Claude Team.</p></div>
            <div class="it neg"><i class="sv"></i><b>Antigravity</b><button>Sign in</button><p>The login expired; its limits cannot be read.</p></div>
            <div class="it neg"><i class="sv"></i><b>Groq</b><span class="est">retry ${dur(4 * 60e3)}</span><p>The router paused it after 3 errors in a row.</p></div></div></div>
          <div><div class="ch"><h2>Next resets</h2><span class="note r">measured</span></div><div class="sched">${resets.map((b) => `<div><time>${at(b.reset).replace(/^\w+ /, '')}</time><span>${esc(b.name)}</span><em>${esc(b.win)}</em></div>`).join('')}</div></div>
          <div><div class="ch"><h2>Running now</h2><span class="note r">${running.length} sessions</span></div><div class="runs">${running.map((r) => `<div><i style="background:${pcCol(r.device)}"></i><span><b>${esc(r.project)}</b> <small>${pcName(r.device)}, ${esc(r.account)}</small></span><time>${dur(r.since)}</time></div>`).join('')}</div></div>
        </div>
      </section>
      ${limits()}
      <section class="blk low">
        <div class="cell hm"><div class="ch"><h2>Activity, 53 weeks</h2><span class="note">daily tokens, every computer · outlined: the busiest day</span></div><div class="hmhost"></div>
          <div class="hmleg"><span>Less</span>${[0, 1, 2, 3, 4].map((i) => `<i style="background:var(--h${i})"></i>`).join('')}<span>More</span></div></div>
        <div class="cell bypc"><div class="ch"><h2>By computer</h2><span class="note">this year</span></div>
          ${computers.map((c, i) => { const v = [0.6, 0.3, 0.1][i]; return `<div class="row"><span>${c.name}${c.me ? ' <small class="est">this one</small>' : ''}</span><span class="mono">${Math.round(v * 100)}%</span><div class="bar"><i style="width:${v * 100}%;background:${c.col}"></i></div></div>`; }).join('')}</div>
      </section>
      <section class="blk"><div class="cell"><div class="ch"><h2>Latest replies</h2><span class="note">${kpi.requests.v.toLocaleString()} today</span><span class="r note">speed and first reply are approximate</span></div>
        <table class="rep"><thead><tr><th>Time</th><th>Account</th><th>Computer</th><th>Model</th><th class="n">Tokens</th><th class="n">Cost</th><th class="n">≈ Speed</th><th class="n">≈ First reply</th></tr></thead><tbody>
        ${replies.slice(0, 8).map((r) => `<tr><td class="tm">${clock(r.ts)}</td><td>${esc(r.account)}</td><td><span class="tl"><i style="background:${pcCol(r.device)}"></i>${pcName(r.device)}</span></td><td class="mono" style="font-size:var(--fs-xs)">${r.model}</td><td class="n">${fmt(r.input + r.output + r.cacheRead + r.cacheWrite)}</td><td class="n">${money(r.cost, 3)}</td><td class="n">${r.tps} tok/s</td><td class="n" style="${r.first > 4 ? 'color:var(--warn)' : ''}">${r.first} s</td></tr>`).join('')}</tbody></table>
        <div class="evid"><span><i></i>Measured</span><span><i class="e"></i>≈ Estimated at this pace</span><span>— Not read yet</span></div></div></section>`;
    app.innerHTML = shell(main);

    // the schematic is drawn for the width it gets
    const host = app.querySelector('.schhost'), W = host.clientWidth;
    const S = schematic(W);
    host.innerHTML = S.svg;
    const svg = host.querySelector('svg'), cell = app.querySelector('.schem');
    const pathEl = (a, b) => svg.querySelector(`path.e[data-a="${a}"][data-b="${b}"]`);
    callout(svg, pathEl('claude', 'anthropic'), 0.5, 26, -50, ['1,680 tok/s', 'first chunk 1.1 s']);
    callout(svg, pathEl('router', 'groq'), 0.9, -196, -6, ['paused', 'retry in 4 min'], 'neg');
    callout(svg, pathEl('antigravity', 'agy'), 0.55, 22, 14, ['login expired'], 'neg');
    // trace a box's routes
    svg.querySelectorAll('.n').forEach((g) => {
      const id = g.dataset.node;
      const on = () => {
        const lit = new Set([id]); let grew = true;
        while (grew) { grew = false; for (const [a, b] of edges) if ((lit.has(a) && !lit.has(b) && (a === id || tools.some((t) => t.id === a) || a === 'router')) || (lit.has(b) && !lit.has(a) && (b === id || tools.some((t) => t.id === b) || b === 'router'))) { lit.add(a); lit.add(b); grew = true; } }
        cell.classList.add('focus');
        svg.querySelectorAll('path.e').forEach((p) => p.classList.toggle('lit', lit.has(p.dataset.a) && lit.has(p.dataset.b)));
        svg.querySelectorAll('.n').forEach((n) => n.classList.toggle('lit', lit.has(n.dataset.node)));
      };
      const off = () => { cell.classList.remove('focus'); svg.querySelectorAll('.lit').forEach((x) => x.classList.remove('lit')); };
      g.addEventListener('mouseenter', on); g.addEventListener('focus', on); g.addEventListener('mouseleave', off); g.addEventListener('blur', off);
    });
    app.querySelector('.hmhost').innerHTML = heat(app.querySelector('.hmhost').clientWidth);

    // motion: the routes are drawn in once, then beads travel as fast as the traffic on them
    const live = S.paths.filter((p) => p.rate > 0).map((p) => ({ ...p, el: pathEl(p.a, p.b) }));
    live.forEach((p) => { p.len = p.el.getTotalLength(); p.n = Math.max(1, Math.round(Math.log10(p.rate) * 1.4)); p.v = 26 + Math.log10(p.rate) * 22; });
    const beadG = svg.querySelector('.beads');
    const beads = [];
    live.forEach((p) => { for (let i = 0; i < p.n; i++) { const c = document.createElementNS('http://www.w3.org/2000/svg', 'rect'); c.setAttribute('class', 'bead ' + p.kind); c.setAttribute('width', 4); c.setAttribute('height', 4); beadG.append(c); beads.push({ p, c, o: i / p.n }); } });
    const place = (t) => beads.forEach((b) => { const u = ((t * b.p.v) / b.p.len + b.o) % 1, pt = b.p.el.getPointAtLength(u * b.p.len); b.c.setAttribute('x', pt.x - 2); b.c.setAttribute('y', pt.y - 2); });
    let raf = 0;
    if (o.motion) {
      svg.querySelectorAll('path.e').forEach((p) => { const L = p.getTotalLength(); p.style.strokeDasharray = p.classList.contains('r') ? '' : L; p.style.strokeDashoffset = p.classList.contains('r') ? '' : L; });
      beadG.style.opacity = 0; svg.querySelector('.callouts').style.opacity = 0;
      requestAnimationFrame(() => {
        svg.querySelectorAll('path.e').forEach((p, i) => { if (p.classList.contains('r')) return; p.style.transition = `stroke-dashoffset 1.1s cubic-bezier(.6,0,.2,1) ${0.15 + (i % 7) * 0.07}s`; p.style.strokeDashoffset = 0; });
        setTimeout(() => { beadG.style.transition = 'opacity .5s'; beadG.style.opacity = 1; svg.querySelector('.callouts').style.transition = 'opacity .5s'; svg.querySelector('.callouts').style.opacity = 1; }, 1300);
      });
      const t0 = performance.now();
      const loop = (n) => { place((n - t0) / 1000); raf = requestAnimationFrame(loop); };
      raf = requestAnimationFrame(loop);
    } else place(3.2);
    return { destroy: () => cancelAnimationFrame(raf) };
  }

  HM.variations.A = { cls: 'vA', render };
})();

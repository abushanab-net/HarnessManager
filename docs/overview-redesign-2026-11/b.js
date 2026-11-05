// B, Orbit (Orbit's mission control, CLUST's callouts, GPUWise's dot-matrix trends), held to the real hops:
// your computers at the centre, then accounts, tools, the router, and the providers outside.
(function () {
  const { NOW, MEASURED, DAY, computers, accounts, tools, providers, router, edges, running, kpi, series, days, replies, fmt, money, dur, at, clock, esc, pct, pcName, pcCol, burn, shell, css, rgb, local } = HM;
  const PI = Math.PI;
  // angles (degrees) chosen so each route runs roughly outward along one bearing
  const ANG = { studio: -40, lab: 80, surface: 200,
    'claude-max': -40, 'claude-team': -6, 'codex-pro': 66, 'opencode-routed': 136, 'pi-openrouter': 186, antigravity: 268,
    claude: -24, codex: 66, opencode: 140, pi: 188, agy: 268, router: 164,
    anthropic: -24, openai: 66, deepseek: 128, openrouter: 164, groq: 200, google: 268 };
  const RING = { pc: 0.13, account: 0.42, tool: 0.63, router: 0.8, provider: 1 };

  function graph() {
    const N = [], E = [];
    const tight = (a) => (a.windows ? a.windows.slice().sort((x, y) => x.used - y.used).pop() : null);
    for (const c of computers) N.push({ id: c.id, kind: 'pc', ring: RING.pc, col: c.col, t: c.name, s: c.me ? 'this computer' : '', size: 6 });
    for (const a of accounts) {
      const w = tight(a);
      const s = a.needsLogin ? 'login expired' : a.cap ? `$${a.cap.spent.toFixed(2)} of $${a.cap.cap} today` : w ? (w.out ? `${w.label.toLowerCase()} ${w.used}%, ≈ out ${at(w.out)}` : `${w.label.toLowerCase()} ${w.used}%, resets ${at(w.reset)}`) : 'API key, no limit';
      N.push({ id: a.id, kind: 'account', ring: RING.account, t: a.label, s, tone: a.needsLogin ? 'neg' : w && w.out ? 'warn' : a.tps ? 'ok' : '', k: a.needsLogin ? 'r' : a.kind === 'routed' ? 'a' : 'c', size: 4 + Math.log10(a.tps + 1) * 2.2, lim: w ? { used: w.used / 100, at: (w.at || w.used) / 100, out: !!w.out } : a.cap ? { used: a.cap.spent / a.cap.cap, at: null } : null, on: a.on });
      for (const pc of a.on) E.push([pc, a.id, 'pc:' + pc, a.tps / a.on.length]);
    }
    for (const t of tools) N.push({ id: t.id, kind: 'tool', ring: RING.tool, t: t.label, s: t.ver, size: 4.5 });
    N.push({ id: 'router', kind: 'router', ring: RING.router, t: 'Local router', s: `:${router.port}, ${router.errors} errors`, size: 8, k: 'a' });
    for (const p of providers) N.push({ id: p.id, kind: 'provider', ring: RING.provider, t: p.label, s: p.paused ? `paused, retry ${dur(p.retry)}` : p.tps ? `${fmt(p.tps)} tok/s, ${p.first} s` : 'no traffic', tone: p.paused ? 'neg' : p.tps ? 'ok' : '', k: p.paused ? 'r' : p.via === 'router' ? 'a' : 'c', size: 4 + Math.log10(p.tps + 1) * 1.9 });
    for (const e of edges) E.push(e);
    N.forEach((n) => { n.ang = (ANG[n.id] * PI) / 180; });
    return { N, E, by: Object.fromEntries(N.map((n) => [n.id, n])) };
  }
  // what a hovered body touches: everything upstream and downstream of it
  function chain(E, id) {
    const lit = new Set([id]);
    const walk = (from, dir) => { const q = [from]; while (q.length) { const x = q.shift(); for (const e of E) { const [a, b] = dir ? [e[0], e[1]] : [e[1], e[0]]; if (a === x && !lit.has(b)) { lit.add(b); q.push(b); } } } };
    walk(id, true); walk(id, false);
    return lit;
  }

  // ---------- the WebGL sky ----------
  function sky(cv, theme) {
    const gl = cv.getContext('webgl', { alpha: false, antialias: false });
    if (!gl) return null;
    const sh = (t, s) => { const x = gl.createShader(t); gl.shaderSource(x, s); gl.compileShader(x); return x; };
    const p = gl.createProgram();
    gl.attachShader(p, sh(gl.VERTEX_SHADER, 'attribute vec2 q;void main(){gl_Position=vec4(q,0.,1.);}'));
    gl.attachShader(p, sh(gl.FRAGMENT_SHADER, `precision highp float;uniform vec2 r;uniform float t,L;uniform vec3 a,b,g;
      float h(vec2 p){return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453);}
      float n(vec2 p){vec2 i=floor(p),f=fract(p);f=f*f*(3.-2.*f);return mix(mix(h(i),h(i+vec2(1,0)),f.x),mix(h(i+vec2(0,1)),h(i+vec2(1,1)),f.x),f.y);}
      float fbm(vec2 p){float v=0.,s=.5;for(int i=0;i<5;i++){v+=s*n(p);p*=2.03;s*=.5;}return v;}
      void main(){vec2 uv=gl_FragCoord.xy/r;vec2 p=(gl_FragCoord.xy-.5*r)/r.y;
        float f=fbm(p*1.5+vec2(t*.010,-t*.007));float f2=fbm(p*3.0-vec2(t*.008,0.)+f*1.3);
        float neb=smoothstep(.38,.98,f2)*(1.-length(p)*.85);float haze=smoothstep(.45,1.,f);
        vec3 col;
        if(L<.5){col=g+a*neb*.20+b*haze*.16;
          vec2 sp=gl_FragCoord.xy/3.;vec2 ci=floor(sp);float st=h(ci);float tw=.55+.45*sin(t*1.7+st*60.);
          if(st>.9972)col+=vec3(.75,.92,1.)*tw*smoothstep(.55,0.,length(fract(sp)-.5));
          col*=1.-.38*pow(length(uv-.5)*1.3,2.);}
        else{col=mix(g,a,neb*.10)-vec3(.03)*haze;col*=1.-.06*pow(length(uv-.5)*1.4,2.);}
        gl_FragColor=vec4(col,1.);}`));
    gl.linkProgram(p); gl.useProgram(p);
    const buf = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const q = gl.getAttribLocation(p, 'q'); gl.enableVertexAttribArray(q); gl.vertexAttribPointer(q, 2, gl.FLOAT, false, 0, 0);
    const U = (k) => gl.getUniformLocation(p, k), c = (x) => rgb(x).map((v) => v / 255);
    const light = theme === 'light';
    gl.uniform3fv(U('a'), c(light ? '#0E7E88' : '#1FA7A3')); gl.uniform3fv(U('b'), c(light ? '#273C75' : '#3B4FA0')); gl.uniform3fv(U('g'), c(light ? '#EEF3F4' : '#040E12')); gl.uniform1f(U('L'), light ? 1 : 0);
    return (t, w, hh) => { if (cv.width !== w || cv.height !== hh) { cv.width = w; cv.height = hh; } gl.viewport(0, 0, w, hh); gl.uniform2f(U('r'), w, hh); gl.uniform1f(U('t'), t); gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4); };
  }

  // ---------- small instruments ----------
  function dots(vals, color, cols = 16, rows = 6) {
    const max = Math.max(...vals), w = cols * 6, hh = rows * 6;
    let s = `<svg width="${w}" height="${hh}" viewBox="0 0 ${w} ${hh}" aria-hidden="true">`;
    vals.slice(-cols).forEach((v, c) => { const lit = Math.max(1, Math.round((v / max) * rows)); for (let r = 0; r < rows; r++) s += `<circle cx="${c * 6 + 3}" cy="${hh - r * 6 - 3}" r="1.7" fill="${r < lit ? color : 'var(--line)'}"${r < lit && c === cols - 1 ? '' : ''}/>`; });
    return s + '</svg>';
  }
  const arc = (cx, cy, r, f0, f1) => {
    if (f1 - f0 >= 0.999) f1 = f0 + 0.999;
    const a0 = f0 * 2 * PI - PI / 2, a1 = f1 * 2 * PI - PI / 2;
    return `M${cx + r * Math.cos(a0)} ${cy + r * Math.sin(a0)} A${r} ${r} 0 ${f1 - f0 > 0.5 ? 1 : 0} 1 ${cx + r * Math.cos(a1)} ${cy + r * Math.sin(a1)}`;
  };
  function rings() {
    return accounts.map((a) => {
      const S = 84, c = 42;
      const tone = (u) => (u >= 80 ? 'var(--neg)' : u >= 55 ? 'var(--warn)' : 'var(--ac)');
      let g = '', note = '', mid = '', cls = '';
      if (a.needsLogin) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--neg)" stroke-dasharray="3 5" opacity=".7"/>`; mid = '—'; note = 'sign in to read'; cls = 'neg'; }
      else if (a.cap) { const u = a.cap.spent / a.cap.cap; g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, 34, 0, u)}" fill="none" stroke="var(--iris)" stroke-width="6" stroke-linecap="round"/>`; mid = Math.round((1 - u) * 100) + '%'; note = `daily cap, resets 00:00`; }
      else if (!a.windows) { g = `<circle cx="${c}" cy="${c}" r="34" fill="none" stroke="var(--line)" stroke-dasharray="2 4"/>`; mid = '∞'; note = 'API key, no limit'; }
      else {
        a.windows.forEach((w, i) => { const r = i ? 34 : 25, u = w.used / 100, f = Math.min(1, (w.at || w.used) / 100);
          g += `<circle cx="${c}" cy="${c}" r="${r}" fill="none" stroke="var(--line)" stroke-width="6"/><path d="${arc(c, c, r, u, f)}" fill="none" stroke="${tone(w.at)}" stroke-width="6" stroke-dasharray="2 3" opacity=".55"/><path d="${arc(c, c, r, 0, u)}" fill="none" stroke="${tone(w.used)}" stroke-width="6" stroke-linecap="round"/>`; });
        const wk = a.windows[1]; mid = 100 - wk.used + '%';
        note = wk.out ? `≈ out ${at(wk.out)}` : `week resets ${at(wk.reset)}`; cls = wk.out ? 'warn' : '';
      }
      return `<div class="ring"><svg width="${S}" height="${S}" viewBox="0 0 ${S} ${S}" role="img" aria-label="${esc(a.label)}: ${esc(note)}">${g}<text x="${c}" y="${c + 5}" text-anchor="middle" font-size="14" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${mid}</text></svg><b>${esc(a.label)}</b><small class="${cls}">${esc(note)}</small></div>`;
    }).join('');
  }
  // the year clock: 53 weeks around, Monday inside to Sunday outside
  function yearClock() {
    const S = 300, c = 150, r0 = 58, r1 = 138, band = (r1 - r0) / 7;
    const nz = days.map((d) => d.tokens).filter(Boolean).sort((a, b) => a - b), q = (p) => nz[Math.floor(p * (nz.length - 1))], cuts = [q(0.25), q(0.5), q(0.75)], top = Math.max(...nz);
    const lv = (v) => (!v ? 0 : v >= top ? 4 : v <= cuts[0] ? 1 : v <= cuts[1] ? 2 : v <= cuts[2] ? 3 : 4);
    const op = [0, 0.28, 0.5, 0.75, 1];
    const off = (days[0].day.getDay() + 6) % 7, weeks = Math.ceil((days.length + off) / 7);
    const P = (r, a) => `${(c + r * Math.cos(a)).toFixed(2)} ${(c + r * Math.sin(a)).toFixed(2)}`;
    let s = `<svg viewBox="0 0 ${S} ${S}" role="img" aria-label="Daily tokens over 53 weeks, as a year clock">`;
    let lastM = -1, best = days.reduce((a, d) => (d.tokens > a.tokens ? d : a));
    days.forEach((d, i) => {
      const w = Math.floor((i + off) / 7), r = (i + off) % 7, a0 = (w / weeks) * 2 * PI - PI / 2 + 0.004, a1 = ((w + 1) / weeks) * 2 * PI - PI / 2 - 0.004;
      const ra = r0 + r * band + 0.6, rb = r0 + (r + 1) * band - 0.6, l = lv(d.tokens);
      s += `<path d="M${P(ra, a0)} A${ra} ${ra} 0 0 1 ${P(ra, a1)} L${P(rb, a1)} A${rb} ${rb} 0 0 0 ${P(rb, a0)} Z" fill="${l ? 'var(--ac)' : 'var(--hm0)'}" fill-opacity="${l ? op[l] : 1}"${d === best ? ' stroke="var(--ink)" stroke-width="1.2"' : ''}><title>${d.day.toDateString()}: ${fmt(d.tokens)} tokens</title></path>`;
      if (r === 0 && d.day.getMonth() !== lastM) { lastM = d.day.getMonth(); const am = (w / weeks) * 2 * PI - PI / 2; s += `<text x="${(c + (r1 + 9) * Math.cos(am)).toFixed(1)}" y="${(c + (r1 + 9) * Math.sin(am) + 3).toFixed(1)}" font-size="8.5" text-anchor="middle" fill="var(--m)" font-family="var(--mono)">${d.day.toLocaleDateString('en-GB', { month: 'short' })[0]}</text>`; }
    });
    s += `<text x="${c}" y="${c - 8}" text-anchor="middle" font-size="11" fill="var(--m)" font-family="var(--sans)">busiest day</text><text x="${c}" y="${c + 10}" text-anchor="middle" font-size="15" font-weight="600" fill="var(--ink)" font-family="var(--sans)">${best.day.toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}</text><text x="${c}" y="${c + 25}" text-anchor="middle" font-size="10.5" fill="var(--m)" font-family="var(--mono)">${fmt(best.tokens)}</text>`;
    return s + '</svg>';
  }

  function render(app, o) {
    const cw = burn().find((b) => b.out), live = accounts.reduce((a, x) => a + x.tps, 0);
    const main = `
      <section class="scene" aria-label="Live routes">
        <canvas class="sky" aria-hidden="true"></canvas><canvas class="orb" aria-hidden="true"></canvas><div class="labels"></div>
        <div class="ov l">
          <div><h1>Overview, live</h1><div class="tps">${live.toLocaleString()}<small>tok/s</small></div><div class="tpsn">last 5 min on 3 computers, read ${clock(MEASURED).slice(0, 5)}</div></div>
          <div class="rd"><label>Tokens today</label><b class="d">${pct(kpi.tokens.v, kpi.tokens.prev)}</b><strong>${fmt(kpi.tokens.v)}</strong>${dots(series.tokens, 'var(--ac)')}</div>
          <div class="rd"><label>Cost at list prices</label><b class="d warn">+${money(kpi.cost.v - kpi.cost.prev)}</b><strong>${money(kpi.cost.v, 0)}</strong>${dots(series.cost, 'var(--warn)')}</div>
          <div class="rd"><label>Read from cache</label><b class="d">+${Math.round((kpi.hit.v - kpi.hit.prev) * 100)} pt</b><strong>${Math.round(kpi.hit.v * 100)}<small>%</small></strong>${dots(series.hit.map((x) => x - 0.8), 'var(--ac)')}</div>
        </div>
        <div class="ov r">
          <div class="inc hd">Needs attention<span>3 open</span></div>
          <div class="inc"><div class="t"><i></i>${esc(cw.name)} weekly<em>≈ ${at(cw.out)}</em></div><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace.</p></div>
          <div class="inc neg"><div class="t"><i></i>Antigravity<em>login</em></div><p>The login expired, so its limits cannot be read.</p><button>Sign in</button></div>
          <div class="inc neg"><div class="t"><i></i>Groq<em>retry ${dur(4 * 60e3)}</em></div><p>Paused by the router after 3 errors in a row.</p></div>
        </div>
        <div class="ov b"><span><i style="background:var(--ac)"></i>signed-in plan</span><span><i style="background:var(--iris)"></i>through the router</span><span><i style="background:var(--neg)"></i>paused or logged out</span>${computers.map((c) => `<span><i style="background:${c.col}"></i>${c.name}</span>`).join('')}</div>
        <div class="ov h">drag to turn, hover to trace</div>
      </section>
      <div class="row3">
        <section class="pn attn2"><div class="ph"><h2>Needs attention</h2><span class="r">3 open</span></div><div class="attn2g">
          <div class="inc"><div class="t"><i></i>${esc(cw.name)} weekly<em>≈ ${at(cw.out)}</em></div><p>Runs out ${dur(cw.reset - cw.out)} before its reset at today's pace.</p></div>
          <div class="inc neg"><div class="t"><i></i>Antigravity<em>login</em></div><p>The login expired, so its limits cannot be read.</p><button>Sign in</button></div>
          <div class="inc neg"><div class="t"><i></i>Groq<em>retry ${dur(4 * 60e3)}</em></div><p>Paused by the router after 3 errors in a row.</p></div></div></section>
        <section class="pn"><div class="ph"><h2>Limits</h2><span>inner 5-hour, outer weekly · dashed is the estimate at the reset</span></div><div class="rings">${rings()}</div></section>
        <section class="pn"><div class="ph"><h2>Running now</h2><span class="r">${running.length} sessions</span></div><div class="runs">${running.map((r) => `<div><i style="background:${pcCol(r.device)}"></i><span><b>${esc(r.project)}</b> <small>${esc(r.account)}, ${pcName(r.device)}</small></span><time>${dur(r.since)}</time></div>`).join('')}</div></section>
        <section class="pn"><div class="ph"><h2>Activity</h2><span class="r">53 weeks, Mon inside</span></div><div class="clock">${yearClock()}</div></section>
      </div>
      <section class="pn wide"><div class="ph"><h2>Latest replies</h2><span>${kpi.requests.v.toLocaleString()} today · speed and first reply are approximate</span></div>
        <table class="rep"><thead><tr><th>Time</th><th>Account</th><th>Model</th><th>Computer</th><th class="n">Tokens</th><th class="n">Cost</th><th class="n">≈ Speed</th><th class="n">≈ First reply</th></tr></thead><tbody>
        ${replies.slice(0, 8).map((r) => `<tr><td class="tm">${clock(r.ts)}</td><td>${esc(r.account)}</td><td class="mono" style="font-size:var(--fs-xs)">${r.model}</td><td><span class="chip"><i style="background:${pcCol(r.device)}"></i>${pcName(r.device)}</span></td><td class="n">${fmt(r.input + r.output + r.cacheRead + r.cacheWrite)}</td><td class="n">${money(r.cost, 3)}</td><td class="n">${r.tps} tok/s</td><td class="n" style="${r.first > 4 ? 'color:var(--warn)' : ''}">${r.first} s</td></tr>`).join('')}</tbody></table></section>`;
    app.innerHTML = shell(main);

    const scene = app.querySelector('.scene'), cvS = scene.querySelector('.sky'), cv = scene.querySelector('.orb'), lab = scene.querySelector('.labels');
    const ctx = cv.getContext('2d'), drawSky = sky(cvS, o.theme);
    const { N, E, by } = graph();
    const C = { ac: css(app, '--ac'), iris: css(app, '--iris'), neg: css(app, '--neg'), warn: css(app, '--warn'), ink: css(app, '--ink'), m: css(app, '--m') };
    const dark = o.theme === 'dark';
    const colOf = (k) => (k && k.startsWith('pc:') ? pcCol(k.slice(3)) : k === 'a' ? C.iris : k === 'r' ? C.neg : C.ac);
    const nodeCol = (n) => (n.kind === 'pc' ? n.col : n.kind === 'tool' ? C.m : n.kind === 'router' ? C.iris : n.k === 'r' ? C.neg : n.kind === 'provider' && !n.size ? C.m : n.tone === '' && n.kind === 'provider' ? C.m : colOf(n.k));
    const rgba = (hex, a) => { const [r, g, b] = rgb(hex); return `rgba(${r},${g},${b},${a})`; };
    // labels, kept as HTML so they stay sharp and readable
    N.forEach((n) => {
      const pc = n.on ? `<span class="pc">${n.on.map((id) => `<i style="background:${pcCol(id)}"></i>`).join('')}</span>` : '';
      n.el = HM.h(`<div class="lb ${n.kind}"><b>${esc(n.t)}${pc}</b>${n.kind === 'tool' ? `<span>${esc(n.s)}</span>` : `<span class="${n.tone || ''}">${esc(n.s)}</span>`}</div>`);
      lab.append(n.el);
    });
    const st = { yaw: -0.25, pitch: 0.78, hover: null, lit: null, drag: null, t: 0 };
    let W = 0, Hh = 0, dpr = 1, R = 0, cx = 0, cy = 0;
    const size = () => {
      W = scene.clientWidth; Hh = scene.clientHeight; dpr = Math.min(2, window.devicePixelRatio || 1);
      cv.width = Math.round(W * dpr); cv.height = Math.round(Hh * dpr);
      const left = app.querySelector('.ov.l').offsetWidth + 24, right = app.querySelector('.ov.r').offsetWidth ? app.querySelector('.ov.r').offsetWidth + 24 : 24;
      cx = left + (W - left - right) / 2; cy = Hh * 0.5;
      R = Math.min((W - left - right) / 2 - 105, (Hh / 2 - 46) / Math.sin(st.pitch + 0.12));
    };
    size();
    const P3 = (r, ang, y = 0) => {
      const a = ang + st.yaw, x = r * R * Math.cos(a), z = r * R * Math.sin(a);
      const yy = y * Math.cos(st.pitch) - z * Math.sin(st.pitch), zz = y * Math.sin(st.pitch) + z * Math.cos(st.pitch), F = 3.2 * R, s = F / (F - zz);
      return { x: cx + x * s, y: cy - yy * s, z: zz, s };
    };
    // a route between two bodies lifts a little off the plane, more for longer hops
    const curve = (A, B, u) => {
      const ax = A.ring * Math.cos(A.ang), az = A.ring * Math.sin(A.ang), bx = B.ring * Math.cos(B.ang), bz = B.ring * Math.sin(B.ang);
      const mx = (ax + bx) / 2, mz = (az + bz) / 2, lift = Math.hypot(bx - ax, bz - az) * 0.22 * R;
      const v = 1 - u, x = v * v * ax + 2 * v * u * mx + u * u * bx, z = v * v * az + 2 * v * u * mz + u * u * bz, y = 2 * v * u * lift;
      const r = Math.hypot(x, z), ang = Math.atan2(z, x);
      return P3(r, ang, y);
    };
    const parts = E.map((e) => ({ e, A: by[e[0]], B: by[e[1]], n: e[3] > 0 ? Math.max(2, Math.min(12, Math.round(Math.log10(e[3]) * 3))) : 0, v: e[3] > 0 ? 0.1 + Math.log10(e[3]) * 0.045 : 0 }));

    function frame(t) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, W, Hh);
      if (drawSky) drawSky(t, Math.round(W * Math.min(2, window.devicePixelRatio || 1)), Math.round(Hh * Math.min(2, window.devicePixelRatio || 1)));
      const lit = st.lit;
      // rings: the hops
      for (const [k, r] of Object.entries(RING)) {
        if (k === 'router') continue;
        ctx.beginPath();
        for (let i = 0; i <= 120; i++) { const p = P3(r, (i / 120) * 2 * PI); i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y); }
        ctx.strokeStyle = rgba(C.ink, dark ? 0.08 : 0.1); ctx.lineWidth = 1; ctx.setLineDash(k === 'tool' ? [2, 5] : []); ctx.stroke(); ctx.setLineDash([]);
      }
      // routes
      for (const p of parts) {
        const on = !lit || (lit.has(p.e[0]) && lit.has(p.e[1]));
        ctx.beginPath();
        for (let i = 0; i <= 28; i++) { const q = curve(p.A, p.B, i / 28); i ? ctx.lineTo(q.x, q.y) : ctx.moveTo(q.x, q.y); }
        const broken = p.e[2] === 'r';
        ctx.strokeStyle = rgba(colOf(p.e[2]), (broken ? 0.35 + 0.25 * Math.sin(t * 5) : p.e[3] ? 0.28 : 0.1) * (on ? 1 : 0.2));
        ctx.lineWidth = on && lit ? 1.6 : 1; ctx.setLineDash(broken ? [3, 4] : []); ctx.stroke(); ctx.setLineDash([]);
      }
      ctx.globalCompositeOperation = dark ? 'lighter' : 'source-over';
      for (const p of parts) {
        if (!p.n) continue;
        const on = !lit || (lit.has(p.e[0]) && lit.has(p.e[1])), col = colOf(p.e[2]);
        for (let i = 0; i < p.n; i++) {
          const u = (t * p.v + i / p.n) % 1;
          for (let j = 0; j < 5; j++) { const uu = u - j * 0.012; if (uu < 0) continue; const q = curve(p.A, p.B, uu); ctx.beginPath(); ctx.arc(q.x, q.y, (2.4 - j * 0.38) * q.s, 0, 2 * PI); ctx.fillStyle = rgba(col, (0.95 - j * 0.18) * (on ? 1 : 0.15)); ctx.fill(); }
        }
      }
      ctx.globalCompositeOperation = 'source-over';
      // bodies, back to front
      const placed = N.map((n) => ({ n, p: P3(n.ring, n.ang) })).sort((a, b) => a.p.z - b.p.z);
      for (const { n, p } of placed) {
        const on = !lit || lit.has(n.id), col = nodeCol(n), r = n.size * p.s;
        if (n.kind !== 'tool') {
          const g = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, r * 3.4);
          g.addColorStop(0, rgba(col, (dark ? 0.42 : 0.22) * (on ? 1 : 0.2))); g.addColorStop(1, rgba(col, 0));
          ctx.fillStyle = g; ctx.beginPath(); ctx.arc(p.x, p.y, r * 3.4, 0, 2 * PI); ctx.fill();
        }
        ctx.beginPath(); ctx.arc(p.x, p.y, r, 0, 2 * PI);
        ctx.fillStyle = n.kind === 'tool' ? (dark ? '#0B1C22' : '#F7FAFA') : rgba(col, on ? 1 : 0.3); ctx.fill();
        if (n.kind === 'tool' || n.kind === 'router') { ctx.lineWidth = 1.4; ctx.strokeStyle = rgba(col, on ? 0.9 : 0.25); ctx.stroke(); }
        if (n.kind === 'pc' && n.id === 'studio') { ctx.beginPath(); ctx.arc(p.x, p.y, r + 4 + 3 * Math.sin(t * 2.2), 0, 2 * PI); ctx.strokeStyle = rgba(col, 0.4); ctx.lineWidth = 1; ctx.stroke(); }
        // the account's tightest limit as an arc around it: solid used, faint to the estimate at its reset
        if (n.lim) {
          const rr = r + 6, a0 = -PI / 2, tone = n.lim.out ? C.warn : n.k === 'a' ? C.iris : C.ac;
          ctx.lineWidth = 2.2; ctx.lineCap = 'round';
          ctx.beginPath(); ctx.arc(p.x, p.y, rr, 0, 2 * PI); ctx.strokeStyle = rgba(C.ink, 0.1); ctx.stroke();
          if (n.lim.at) { ctx.beginPath(); ctx.arc(p.x, p.y, rr, a0, a0 + 2 * PI * Math.min(1, n.lim.at)); ctx.strokeStyle = rgba(tone, 0.3); ctx.stroke(); }
          ctx.beginPath(); ctx.arc(p.x, p.y, rr, a0, a0 + 2 * PI * n.lim.used); ctx.strokeStyle = rgba(tone, on ? 1 : 0.3); ctx.stroke(); ctx.lineCap = 'butt';
        }
        // the label beside it, on the outer side
        const leftSide = p.x < cx - 4, off = r + (n.lim ? 12 : 7);
        n.el.style.transform = `translate(${(leftSide ? p.x - off : p.x + off).toFixed(1)}px, ${(p.y - 9).toFixed(1)}px)${leftSide ? ' translateX(-100%)' : ''}`;
        n.el.style.textAlign = leftSide ? 'right' : 'left';
        n.el.style.zIndex = Math.round(p.z + 1000); n.el.style.display = n.kind === 'tool' && !(lit && lit.has(n.id)) ? 'none' : '';
        n.el.style.opacity = (lit ? (on ? 1 : 0.18) : 1) * (0.55 + 0.45 * Math.min(1, (p.z + R) / (1.2 * R)));
      }
    }
    // hover traces a body's whole route; drag turns the scene
    const near = (x, y) => { let best = null, bd = 22; for (const n of N) { const p = P3(n.ring, n.ang), d = Math.hypot(p.x - x, p.y - y); if (d < bd) { bd = d; best = n; } } return best; };
    cv.addEventListener('pointermove', (e) => {
      const [x, y] = local(cv, e);
      if (st.drag) { st.yaw += (x - st.drag[0]) * 0.008; st.pitch = Math.max(0.3, Math.min(1.3, st.pitch + (y - st.drag[1]) * 0.004)); st.drag = [x, y]; if (!o.motion) frame(st.t); return; }
      const n = near(x, y);
      if ((n && n.id) !== st.hover) { st.hover = n && n.id; st.lit = n ? chain(E, n.id) : null; if (!o.motion) frame(st.t); }
    });
    cv.addEventListener('pointerdown', (e) => { st.drag = local(cv, e); cv.classList.add('drag'); cv.setPointerCapture(e.pointerId); });
    cv.addEventListener('pointerup', () => { st.drag = null; cv.classList.remove('drag'); });
    cv.addEventListener('pointerleave', () => { st.hover = null; st.lit = null; if (!o.motion) frame(st.t); });

    let raf = 0, last = performance.now(), visible = true;
    const io = new IntersectionObserver((en) => { visible = en[0].isIntersecting; }, { threshold: 0.05 });
    io.observe(scene);
    const ro = new ResizeObserver(() => { size(); if (!o.motion) frame(st.t); });
    ro.observe(scene);
    if (o.motion) {
      const loop = (now) => {
        const dt = Math.min(0.05, (now - last) / 1000); last = now;
        if (visible) { if (!st.drag && !st.hover) st.yaw += dt * 0.045; st.t += dt; frame(st.t); }
        raf = requestAnimationFrame(loop);
      };
      raf = requestAnimationFrame(loop);
    } else { st.t = 4; frame(st.t); }
    return { destroy: () => { cancelAnimationFrame(raf); io.disconnect(); ro.disconnect(); } };
  }

  HM.variations.B = { cls: 'vB', render };
})();

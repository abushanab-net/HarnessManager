// Designer C (c-opencode) — Chrono-Runway View Renderers (Overview, Accounts, First-Run)
(function () {
  const D = window.HM_DATA;

  // Single linear 7-day scale (0 .. 7 days = 0 .. 100%)
  const HORIZON_MS = 7 * D.DAY;
  function linearPct(ms) {
    if (ms == null || ms <= 0) return 0;
    return Math.min(100, Math.max(0, (ms / HORIZON_MS) * 100));
  }

  // Compute the 8 linear ticks (NOW, +1d .. +7d) from D.NOW
  function renderLinearAxisTicks() {
    return [0, 2, 4, 6, 7].map((d) => `<span class="axis-tick ${d === 0 ? 'now' : d === 7 ? 'end' : ''}" style="left:${d / 7 * 100}%">${d === 0 ? 'NOW' : '+' + d + 'd'}</span>`).join('');
  }

  function renderRingSvg(acct, showUsedPct) {
    if (acct.kind === 'apikey') return `<div class="ring-wrap" title="API key · no plan limit"><span class="ring-val">∞</span></div>`;
    if (!acct.measured || !acct.windows || !acct.windows.length) {
      return `<div class="ring-wrap" title="Limits not read (—)">
        <svg viewBox="0 0 32 32"><circle cx="16" cy="16" r="12.5" fill="none" stroke="var(--line-strong)" stroke-width="2.6" stroke-dasharray="3 3"/></svg>
        <span class="ring-val">—</span>
      </div>`;
    }
    const wk = acct.windows.find((w) => w.kind === 'week') || acct.windows[0];
    const pct = showUsedPct ? wk.used : wk.left;
    const c = 2 * Math.PI * 12.5;
    const fillLen = pct === 0 ? 0 : (Math.max(2, pct) / 100) * c;
    const isExhausted = acct.windows.some((w) => w.left === 0);
    const hasOut = acct.windows.some((w) => w.left === 0 || (w.outMs != null && w.outMs < w.resetMs));
    let stroke = 'var(--teal)';
    if (wk.left <= 30 || hasOut) stroke = 'var(--amber)';
    if (isExhausted || acct.needsLogin) stroke = 'var(--coral)';
    return `<div class="ring-wrap" title="${D.esc(acct.name)}: week ${wk.left}% left (${wk.used}% used)">
      <svg viewBox="0 0 32 32">
        <circle cx="16" cy="16" r="12.5" fill="none" stroke="var(--line-default)" stroke-width="2.8"/>
        <circle cx="16" cy="16" r="12.5" fill="none" stroke="${stroke}" stroke-width="2.8" stroke-linecap="round" stroke-dasharray="${fillLen.toFixed(1)} ${c.toFixed(1)}"/>
      </svg>
      <span class="ring-val">${pct}%</span>
    </div>`;
  }

  function renderDotMatrix(arr) {
    return `<div class="dot-matrix" aria-hidden="true">${arr.map((v) => `<i class="dot-cell lv-${v}"></i>`).join('')}</div>`;
  }

  // Crisp radial wedge cell Year Clock (no blur smudge, high dynamic range, 53-week computer totals)
  function renderYearClockSvg(computerFilter, metrics) {
    const cx = 78, cy = 78, rInner = 30, rOuter = 67;
    const months = ['N', 'D', 'J', 'F', 'M', 'A', 'M', 'J', 'J', 'A', 'S', 'O'];
    const parts = [];
    months.forEach((m, idx) => {
      const ang = ((idx / 12) * 2 * Math.PI) - Math.PI / 2;
      const mx = cx + Math.cos(ang) * 73;
      const my = cy + Math.sin(ang) * 73;
      parts.push(`<text x="${mx.toFixed(1)}" y="${my.toFixed(1)}" text-anchor="middle" dominant-baseline="central" fill="var(--ink-muted)" font-size="11" font-weight="600">${m}</text>`);
    });

    const stepA = (2 * Math.PI) / 54; // leaves a clear 1-week "now" gap at 12 o'clock
    const stepR = (rOuter - rInner) / 7;
    D.YEAR_CLOCK_DAYS.forEach((cell) => {
      const val = cell.byComputer[computerFilter] != null ? cell.byComputer[computerFilter] : cell.intensity;
      const a0 = cell.week * stepA - Math.PI / 2 + 0.03;
      const a1 = a0 + stepA * 0.78;
      const r0 = rInner + cell.day * stepR + 0.4;
      const r1 = r0 + stepR - 0.9;
      const x0 = cx + Math.cos(a0) * r0, y0 = cy + Math.sin(a0) * r0;
      const x1 = cx + Math.cos(a1) * r0, y1 = cy + Math.sin(a1) * r0;
      const x2 = cx + Math.cos(a1) * r1, y2 = cy + Math.sin(a1) * r1;
      const x3 = cx + Math.cos(a0) * r1, y3 = cy + Math.sin(a0) * r1;

      let lv = 0;
      if (val >= 0.85) lv = 4;
      else if (val >= 0.55) lv = 3;
      else if (val >= 0.30) lv = 2;
      else if (val >= 0.08) lv = 1;

      parts.push(`<path class="yc-cell lv-${lv}" d="M${x0.toFixed(1)},${y0.toFixed(1)} L${x1.toFixed(1)},${y1.toFixed(1)} L${x2.toFixed(1)},${y2.toFixed(1)} L${x3.toFixed(1)},${y3.toFixed(1)} Z"/>`);
    });

    const selectedPc = D.COMPUTERS.find((c) => c.id === computerFilter);
    const centerVal = selectedPc ? D.fmtTokens(selectedPc.tokens53w) : metrics.peakDayVal;
    const centerSub = selectedPc ? `${selectedPc.name} (53w)` : 'busiest day';
    const centerMain = selectedPc ? '53-wk total' : metrics.peakDayLabel;

    return `<div class="year-clock-svg-box">
      <svg width="156" height="156" viewBox="0 0 156 156" role="img" aria-label="53-week activity radial heatmap">${parts.join('')}</svg>
      <div class="year-clock-center">
        <small>${centerSub}</small>
        <b>${centerMain}</b>
        <span>${centerVal}</span>
      </div>
    </div>`;
  }

  // Column 2: Pure Capacity Left (Zero-baseline fixed-width bars, never multiplied by time)
  function renderCapacityColumn(acct, showUsedPct) {
    if (!acct.measured || !acct.windows || !acct.windows.length) {
      if (acct.cap) {
        const leftUsd = acct.cap.limitUsd - acct.cap.spentUsd;
        const leftPct = Math.round((leftUsd / acct.cap.limitUsd) * 100);
        const shownPct = showUsedPct ? 100 - leftPct : leftPct;
        return `<div class="lane-cap-col" title="Daily router spend cap: $${leftUsd.toFixed(2)} left of $${acct.cap.limitUsd}">
          <div class="cap-mini-row">
            <span class="cap-mini-lbl">Cap</span>
            <div class="cap-mini-track"><div class="cap-mini-fill" style="width:${shownPct}%"></div></div>
            <span class="cap-mini-pct">$${leftUsd.toFixed(0)}</span>
          </div>
          <div style="font-size:11px;color:var(--ink-muted)">— no vendor window</div>
        </div>`;
      }
      return `<div class="lane-cap-col">
        <div style="font-size:11px;color:var(--ink-muted)">${!['claude','codex'].includes(acct.tool) ? 'No plan limits' : '— not read'}</div>
        <div style="font-size:11px;color:${acct.needsLogin ? 'var(--coral)' : 'var(--ink-dim)'}">${acct.needsLogin ? 'Login expired' : !['claude','codex'].includes(acct.tool) ? 'Usage only' : 'Sign in to read'}</div>
      </div>`;
    }

    const w5 = acct.windows.find((w) => w.kind === '5h');
    const wk = acct.windows.find((w) => w.kind === 'week') || acct.windows[0];
    const rows = [];
    if (w5) {
      const val = showUsedPct ? w5.used : w5.left;
      const col = w5.left === 0 ? 'coral' : w5.left <= 25 ? 'amber' : (w5.resetMs <= 2 * D.H && w5.left >= 75) ? 'emerald' : '';
      rows.push(`<div class="cap-mini-row" title="5-hour window: ${w5.left}% left (${w5.used}% used)">
        <span class="cap-mini-lbl">5h</span>
        <div class="cap-mini-track"><div class="cap-mini-fill ${col}" style="width:${val}%"></div></div>
        <span class="cap-mini-pct">${val}%</span>
      </div>`);
    }
    if (wk) {
      const val = showUsedPct ? wk.used : wk.left;
      const col = wk.left === 0 ? 'coral' : (wk.outMs || wk.left <= 30) ? 'amber' : '';
      rows.push(`<div class="cap-mini-row" title="Weekly window: ${wk.left}% left (${wk.used}% used)">
        <span class="cap-mini-lbl">Wk</span>
        <div class="cap-mini-track"><div class="cap-mini-fill ${col}" style="width:${val}%"></div></div>
        <span class="cap-mini-pct">${val}%</span>
      </div>`);
    }
    return `<div class="lane-cap-col">${rows.join('')}</div>`;
  }

  // Column 3: Linear 7-Day Reset Horizon Track (0 .. 168h strictly linear)
  function renderLinearRunwayTrack(acct, state) {
    if (!acct.measured || !acct.windows.length) {
      if (acct.cap) return `<div class="lane-runway cap-runway"><span>Daily cap · ↻ ${D.fmtDuration(acct.cap.resetMs)}</span><span>${D.esc(acct.route || 'Local router')}</span></div>`;
      const noLimits = !['claude', 'codex'].includes(acct.tool) || acct.kind !== 'subscription';
      return `<div class="lane-runway unread-runway">${noLimits ? 'No plan windows' : '— Limits not read'}${acct.needsLogin ? ' · Sign in' : ''}</div>`;
    }
    const w5 = acct.windows.find((w) => w.kind === '5h');
    const wk = acct.windows.find((w) => w.kind === 'week');
    const row = (w, short) => {
      const scale = short ? 5 * D.H : HORIZON_MS;
      const gate = Math.min(100, w.resetMs / scale * 100);
      const out = w.outMs != null && w.outMs < w.resetMs ? Math.min(100, w.outMs / scale * 100) : null;
      return `<div class="runway-instrument ${short ? 'short-track' : ''}" title="${w.label}: reset ${D.fmtAbsoluteReset(w.resetMs)}${out !== null ? '; ≈ out ' + D.fmtAbsoluteReset(w.outMs) : ''}">
        <div class="runway-caption"><span>${short ? '5h · 0–5h zoom' : 'Week · 0–7d'}${short && w.resetMs <= 2 * D.H && w.left >= 75 ? ' · Proposed: harvest' : ''}</span><span>↻ ${D.fmtDuration(w.resetMs)}</span></div>
        <div class="runway-track-row">
          ${out !== null ? `<div class="runway-forecast-dash warn" style="width:${out}%"></div><div class="runway-starve-gap" style="left:${out}%;width:${gate - out}%"></div>` : `<div class="runway-bar-fill teal" style="width:${gate}%"></div>`}
          <div class="reset-gate" style="left:calc(${gate}% - 1px)"></div>
        </div>
      </div>`;
    };
    return `<div class="lane-runway">${w5 && state.windowMode !== 'week' ? row(w5, true) : ''}${wk && state.windowMode !== '5h' ? row(wk, false) : ''}</div>`;
  }

  function renderOverview(state, accounts, metrics) {
    if (!accounts || accounts.length === 0) {
      return renderFirstRunOrEmpty(state, []);
    }
    const search = (state.searchQuery || '').trim().toLowerCase();
    const filtered = accounts.filter((a) => {
      if (state.toolFilter !== 'all' && a.tool !== state.toolFilter) return false;
      if (state.computerFilter === 'this' && !a.computers.includes('developmentpc') && a.openSessions > 0) return false;
      if (search && !a.name.toLowerCase().includes(search) && !a.tool.toLowerCase().includes(search) && !(a.email || '').toLowerCase().includes(search)) return false;
      return true;
    });

    if (!metrics.bestRoomAcct) return renderFirstRunOrEmpty(state, accounts);
    const bestRoomAcct = metrics.bestRoomAcct;
    const expiringAcct = metrics.expiringAcct || bestRoomAcct;
    const primaryPick = state.preferExpiring ? expiringAcct : bestRoomAcct;
    const pickW5 = (primaryPick.windows || []).find((w) => w.kind === '5h');
    const pickWk = (primaryPick.windows || []).find((w) => w.kind === 'week');
    const bestW5 = (bestRoomAcct.windows || []).find((w) => w.kind === '5h');
    const bestWk = (bestRoomAcct.windows || []).find((w) => w.kind === 'week');
    const expW5 = (expiringAcct.windows || []).find((w) => w.kind === '5h');

    const primaryRisk = metrics.primaryRisk;
    const selectedAcct = accounts.find((a) => a.id === state.selectedAccountId) || (primaryRisk ? primaryRisk.acct : accounts[0]);
    const selectedTool = D.TOOLS.find((t) => t.id === selectedAcct.tool) || D.TOOLS[0];

    const displayedSessions = metrics.activeSessions.filter((s) => state.livePcFilter === 'all' || s.computer === state.livePcFilter)
      .slice().sort((a, b) => (a.status === 'streaming' ? 0 : 1) - (b.status === 'streaming' ? 0 : 1));
    if (state.livePcFilter === 'all') {
      const first = metrics.computersList.map((c) => displayedSessions.find((s) => s.computer === c.id)).filter(Boolean);
      displayedSessions.sort((a,b) => (first.includes(a) ? first.indexOf(a) - 100 : 0) - (first.includes(b) ? first.indexOf(b) - 100 : 0));
    }

    const readoutData = D.READOUTS[state.readoutRange] || D.READOUTS.today;

    return `<div class="page-body">
      <!-- ROW 1: THE 3-PART OWNER'S SENTENCE DECK -->
      <section class="dispatch-deck" aria-label="Primary operational summary">
        <!-- Card 1 (HERO LEAD): Which account to use now because it resets soon / has most room -->
        <div class="deck-card hero-lead">
          <div class="eyebrow-row">
            <span class="eyebrow teal">Use next · Reset runway</span>
            <button class="proposed-tag" data-action="toggle-prefer-expiring" title="Proposed feature: let Start automatically pick an account whose 5-hour window resets within 2 hours and still has >50% room">
              Proposed: ${state.preferExpiring ? 'Expiring 5h first ✓' : 'Prefer expiring 5h'}
            </button>
          </div>
          <div class="lead-main">
            <div style="flex:1;min-width:210px">
              <div class="lead-headline">
                Use <span style="color:var(--teal);white-space:nowrap">${D.esc(primaryPick.name)}</span>

              </div>
              <div class="lead-sub">
                ${D.LOGOS[primaryPick.tool]} <b>${ (D.TOOLS.find((t) => t.id === primaryPick.tool) || {}).name }</b> · ${primaryPick.plan}
                ${pickW5 ? ` · 5h ↻ ${D.fmtDuration(pickW5.resetMs)} (${D.fmtAbsoluteReset(pickW5.resetMs)})` : ''}
                · read ${primaryPick.readAgoSec != null ? primaryPick.readAgoSec + 's ago' : 'never'}
              </div>
            </div>
            <div style="display:flex;gap:5px;align-items:center;flex-shrink:0">
              <button class="cta-start-lg" data-action="start-account" data-account="${primaryPick.id}">
                ▶ Start
              </button>
              <button class="btn-secondary" style="padding:6px 8px;font-size:11px" data-action="open-start-folder" data-account="${primaryPick.id}" title="Start in a specific folder with flags">
                Folder…
              </button>
            </div>
          </div>
          <div class="lead-candidates">
            <button class="candidate-pill ${!state.preferExpiring ? 'active' : ''}" data-action="pick-hero-mode" data-expiring="false">
              <div>
                <b style="white-space:nowrap">${D.esc(bestRoomAcct.name)}</b>
                <div style="color:var(--ink-muted)">Most room: ${bestW5 ? bestW5.left + '% 5h · ' : ''}${bestWk ? bestWk.left + '% wk' : ''}</div>
              </div>
              <span style="color:var(--teal);font-weight:700">↻ ${bestW5 ? D.fmtDuration(bestW5.resetMs) : '—'}</span>
            </button>
            <button class="candidate-pill ${state.preferExpiring ? 'active' : ''}" data-action="pick-hero-mode" data-expiring="true">
              <div>
                <b style="white-space:nowrap">${D.esc(expiringAcct.name)}</b>
                <div style="color:var(--ink-muted)">Resets soonest: ${expW5 ? expW5.left + '% 5h left' : ''}</div>
              </div>
              <span style="color:var(--emerald);font-weight:700">↻ ${expW5 ? D.fmtDuration(expW5.resetMs) : '—'}</span>
            </button>
          </div>
        </div>

        <!-- Card 2: Which account is exhausted / running out before its reset -->
        <div class="deck-card exhaust-card">
          <div class="eyebrow-row">
            <span class="eyebrow amber">Runs out first</span>
            <span style="font-size:11px;color:var(--ink-muted)">Measured + ≈ Pace</span>
          </div>
          ${primaryRisk ? `
            <div>
              <div class="exhaust-Finding" id="finding">
                <span class="warn-name" style="white-space:nowrap">${D.esc(primaryRisk.acct.name)}</span>
                ${primaryRisk.win.left === 0
                  ? `is exhausted now (0% ${primaryRisk.win.label} left, resets in ${D.fmtDuration(primaryRisk.win.resetMs)}).`
                  : `will likely run out of its ${primaryRisk.win.label.toLowerCase()} ≈ ${D.fmtAbsoluteReset(primaryRisk.outMs)}, <span style="color:var(--coral)">${D.fmtDuration(primaryRisk.gapMs)}</span> before reset.`}
              </div>
              <div class="exhaust-basis" id="basis" style="margin-top:3px">
                Measured <b>${primaryRisk.win.used}% used</b> (${primaryRisk.win.left}% left), resets <b>${D.fmtAbsoluteReset(primaryRisk.win.resetMs)}</b>, read ${primaryRisk.acct.readAgoSec}s ago · pace forecast ≈.
              </div>
            </div>
          ` : `<div class="exhaust-Finding">No account is on pace to run out before its reset.</div>`}
          <div class="deficit-chips">
            ${metrics.atRisk.slice(0, 3).map((r) => `
              <button class="mini-chip ${state.selectedAccountId === r.acct.id ? 'active' : ''}" data-action="select-lane" data-account="${r.acct.id}">
                ${D.LOGOS[r.acct.tool]} <b style="white-space:nowrap">${D.esc(r.acct.name)} · ${r.win.label}</b>
                <span style="color:var(--amber)">${r.win.left === 0 ? '0% left now' : `${r.win.left}% left · ≈ ${D.fmtAbsoluteReset(r.outMs)}`}</span>
              </button>
            `).join('')}
            ${accounts.filter((a) => a.needsLogin).map((a) => `
              <button class="mini-chip danger ${state.selectedAccountId === a.id ? 'active' : ''}" data-action="select-lane" data-account="${a.id}">
                ${D.LOGOS[a.tool]} <b style="white-space:nowrap">${D.esc(a.name)}</b> <span>— login expired</span>
              </button>
            `).join('')}
          </div>
        </div>

        <!-- Card 3: What is running now, on which computer, follow one -->
        <div class="deck-card">
          <div class="eyebrow-row">
            <span class="eyebrow secondary">Running now</span>
            <span style="font-size:11px;color:var(--ink-muted)">${metrics.liveHere} here · ${metrics.liveElsewhere} elsewhere</span>
          </div>
          <div class="chip-group" role="group" aria-label="Filter running sessions by computer">
            <button class="filter-chip ${state.livePcFilter === 'all' ? 'active' : ''}" style="padding:2px 6px;font-size:11px" data-action="set-live-pc" data-pc="all">All (${metrics.activeSessions.length})</button>
            ${metrics.computersList.map((c) => {
              const cnt = metrics.activeSessions.filter((s) => s.computer === c.id).length;
              return `<button class="filter-chip ${state.livePcFilter === c.id ? 'active' : ''}" style="padding:2px 6px;font-size:11px" data-action="set-live-pc" data-pc="${c.id}"><i class="pc-dot" style="background:${c.col}"></i>${c.name} ${cnt}</button>`;
            }).join('')}
          </div>
          <div class="live-mini-list" tabindex="0" aria-label="All running sessions; scroll for more">
            ${displayedSessions.map((s) => {
              const pc = D.COMPUTERS.find((c) => c.id === s.computer) || D.COMPUTERS[0];
              return `<div class="live-mini-row">
                <div class="live-mini-left">
                  <i class="pc-dot" style="background:${pc.col}" title="${pc.name}"></i>
                  ${D.LOGOS[s.tool]}
                  <b>${s.project}</b>
                  <span class="live-mini-meta">${pc.name}</span><span class="live-account">${s.accountId} · ${s.status} · ${s.sinceMin}m</span>
                </div>
                <button class="btn-follow" data-action="follow-session" data-session="${s.id}">Follow</button>
              </div>`;
            }).join('')}
          </div>
        </div>
      </section>

      <!-- ROW 2 (CENTERPIECE): THE RESET HORIZON TIMELINE -->
      <section class="horizon-panel" aria-label="Reset Horizon Timeline">
        <div class="horizon-header">
          <div class="horizon-title-group">
            <div class="horizon-title">
              <span>Reset Horizon</span>
            </div>
            <div class="honesty-legend">
              <span class="legend-item"><i class="legend-swatch solid"></i>Time until reset</span>
              <span class="legend-item"><i class="legend-swatch dashed"></i>≈ Pace forecast</span>
              <span class="legend-item"><i class="legend-swatch starve"></i>Runs out before reset</span>
              <span class="legend-item"><b style="color:var(--ink-secondary)">—</b> Not read</span>
            </div>
          </div>
          <div class="horizon-controls">
            <div class="chip-group" role="group" aria-label="Window filter">
              <button class="filter-chip ${state.windowMode === 'both' ? 'active' : ''}" data-action="set-win-mode" data-mode="both">5h + Week</button>
              <button class="filter-chip ${state.windowMode === '5h' ? 'active' : ''}" data-action="set-win-mode" data-mode="5h">5-hour only</button>
              <button class="filter-chip ${state.windowMode === 'week' ? 'active' : ''}" data-action="set-win-mode" data-mode="week">Weekly only</button>
            </div>
            <button class="filter-chip active" data-action="toggle-ring-mode" title="Switch limit rings and capacity bars between % left and % used">
              Rings: ${state.showUsedPct ? '% used' : '% left'}
            </button>
          </div>
        </div>

        <!-- Tool & Computer Filter Bar -->
        <div class="horizon-filterbar">
          <div class="chip-group" role="group" aria-label="Filter by tool">
            <button class="filter-chip ${state.toolFilter === 'all' ? 'active' : ''}" data-action="set-tool-filter" data-tool="all">All (${accounts.length})</button>
            ${D.TOOLS.map((t) => {
              const cnt = accounts.filter((a) => a.tool === t.id).length;
              return `<button class="filter-chip ${state.toolFilter === t.id ? 'active' : ''}" data-action="set-tool-filter" data-tool="${t.id}">${D.LOGOS[t.id]}<span>${t.name}</span><span style="opacity:0.75">${cnt}</span></button>`;
            }).join('')}
          </div>
          <div class="chip-group" role="group" aria-label="Filter by computer">
            <button class="filter-chip ${state.computerFilter === 'all' ? 'active' : ''}" data-action="set-pc-filter" data-pc="all">All computers (${metrics.computersList.length})</button>
            <button class="filter-chip ${state.computerFilter === 'this' ? 'active' : ''}" data-action="set-pc-filter" data-pc="this"><i class="pc-dot" style="background:#3FD6CF"></i>This computer (DEVELOPMENTPC)</button>
          </div>
        </div>

        <!-- Separated Capacity Column + Single Linear 7-Day Time Axis Ruler -->
        <div class="timeline-grid-head">
          <div>ACCOUNT &amp; WEEK RING (${state.showUsedPct ? 'USED' : 'LEFT'})</div>
          <div>CAPACITY (${state.showUsedPct ? '% USED' : '% LEFT'})</div>
          <div class="axis-ruler">${renderLinearAxisTicks()}</div>
          <div style="padding-left:6px">NEXT RESET &amp; PACE</div>
          <div style="text-align:right">ACTION</div>
        </div>

        <!-- Account Timeline Lanes -->
        <div class="timeline-lanes">
          ${filtered.map((acct) => {
            const isSel = acct.id === selectedAcct.id;
            const w5 = (acct.windows || []).find((w) => w.kind === '5h');
            const wk = (acct.windows || []).find((w) => w.kind === 'week');
            const pcDots = (acct.computers || []).map((cid) => {
              const pc = D.COMPUTERS.find((c) => c.id === cid);
              return pc ? `<i class="pc-dot" style="background:${pc.col}" title="Active on ${pc.name}"></i>` : '';
            }).join('');

            const noPlan = !['claude','codex'].includes(acct.tool) || acct.kind !== 'subscription';
            let readoutTop = noPlan ? 'No plan limits' : '— not read';
            let readoutSub = acct.needsLogin ? 'Login expired' : noPlan ? 'Usage only' : 'Sign in or refresh';
            let subCls = acct.needsLogin ? 'danger' : '';

            if (acct.measured && wk) {
              readoutTop = w5 ? `5h ↻ ${D.fmtDuration(w5.resetMs)} · Wk ↻ ${D.fmtDuration(wk.resetMs)}` : `Wk ↻ ${D.fmtDuration(wk.resetMs)}`;
              if (w5 && w5.left === 0) {
                readoutSub = `Exhausted (5h ↻ ${D.fmtDuration(w5.resetMs)})`;
                subCls = 'danger';
              } else if (wk.outMs) {
                readoutSub = `≈ Out ${D.fmtAbsoluteReset(wk.outMs)}`;
                subCls = 'warn';
              } else if (w5 && w5.resetMs <= 2 * D.H && w5.left >= 75) {
                readoutSub = `Use ${w5.left}% before ${D.fmtAbsoluteReset(w5.resetMs)}`;
                subCls = 'good';
              } else {
                readoutSub = `≈ ${100 - wk.forecastAtReset}% left at reset`;
                subCls = 'good';
              }
            } else if (acct.cap) {
              readoutTop = `Cap ↻ ${D.fmtDuration(acct.cap.resetMs)}`;
              readoutSub = `$${acct.cap.spentUsd.toFixed(2)} / $${acct.cap.limitUsd} spent`;
            }

            return `<div class="t-lane ${isSel ? 'selected' : ''} ${acct.exhaustionAlert ? 'warn-lane' : ''}" tabindex="0" data-action="select-lane" data-account="${acct.id}">
              <div class="lane-id">
                ${renderRingSvg(acct, state.showUsedPct)}
                <div class="lane-name-box">
                  <div class="lane-name-top">
                    ${D.LOGOS[acct.tool]}
                    <span class="lane-acct-name">${D.esc(acct.name)}</span>
                    <span class="pc-dots">${pcDots}</span>
                  </div>
                  <div class="lane-sub-meta">
                    <span class="plan-pill">${acct.plan || 'Profile'}</span>
                    ${acct.openSessions > 0 ? `<span style="color:var(--teal);display:inline-flex;align-items:center;gap:4px"><i class="live-pulse-dot"></i>${acct.openSessions} open</span>` : `<span>idle</span>`}
                  </div>
                </div>
              </div>
              ${renderCapacityColumn(acct, state.showUsedPct)}
              ${renderLinearRunwayTrack(acct, state)}
              <div class="lane-readout">
                <span class="lane-readout-primary">${readoutTop}</span>
                <span class="lane-readout-sub ${subCls}">${readoutSub}</span>
              </div>
              <div class="lane-action">
                ${acct.needsLogin
                  ? `<button class="btn-lane-start" style="border-color:var(--coral);color:var(--coral)" data-action="reauth-account" data-account="${acct.id}">Sign in</button>`
                  : `<button class="btn-lane-start" data-action="start-account" data-account="${acct.id}">▶ Start</button>`
                }
              </div>
            </div>`;
          }).join('')}
        </div>

        <!-- Active Route Inspector Footer (Computer → Account → Tool (version · first reply ≈ Xs · N open)) -->
        <div class="route-inspector-bar">
          <div class="route-chain">
            <span style="color:var(--ink-muted);font-weight:600">Selected route:</span>
            <span class="route-node">
              <span class="pc-dots">${(selectedAcct.computers.length ? selectedAcct.computers : ['developmentpc']).map((cid) => {
                const pc = D.COMPUTERS.find((c) => c.id === cid) || D.COMPUTERS[0];
                return `<i class="pc-dot" style="background:${pc.col}"></i><span>${pc.name}</span>`;
              }).join(' + ')}</span>
            </span>
            <span class="route-arrow">→</span>
            <span class="route-node">
              ${D.LOGOS[selectedAcct.tool]}
              <b>${D.esc(selectedAcct.name)}</b>
              <span style="color:var(--ink-muted)">(${selectedAcct.measured && selectedAcct.windows[1] ? `${selectedAcct.windows[1].used}% wk used` : '— not read'})</span>
            </span>
            <span class="route-arrow">→</span>
            <span class="route-node">
              <b>${selectedTool.name}</b>
              <span style="color:var(--ink-muted)">v${selectedTool.ver}</span>
              <span style="color:var(--teal)">· first reply ${selectedAcct.firstReplySec ? `≈ ${selectedAcct.firstReplySec}s` : '—'} · ${selectedAcct.openSessions} open</span>
            </span>
          </div>
          <div style="display:flex;align-items:center;gap:8px">
            <span style="font-size:11px;color:var(--ink-muted)"><kbd class="kbd">Esc</kbd> resets selection</span>
            <button class="filter-chip active" data-action="open-account-detail" data-account="${selectedAcct.id}">Inspect ${D.esc(selectedAcct.name)} ↗</button>
          </div>
        </div>
      </section>

      <!-- ROW 3: READOUTS + NEEDS ATTENTION + 53-WEEK YEAR CLOCK -->
      <section class="lower-grid" aria-label="Telemetry readouts, attention items, and 53-week activity">
        <div class="panel-card">
          <div class="panel-head">
            <span class="panel-title">Period Readouts · Dot-Grid Trend</span>
            <div class="chip-group" role="group" aria-label="Readout range">
              ${['today', '7d', '30d', 'all'].map((r) => `<button class="filter-chip ${state.readoutRange === r ? 'active' : ''}" style="padding:2px 7px;font-size:11px" data-action="set-readout-range" data-range="${r}">${D.READOUTS[r].label}</button>`).join('')}
            </div>
          </div>
          <div class="readout-rows">
            <div class="readout-item" title="${readoutData.tokens.sub}">
              <span class="readout-label">Tokens</span>
              <span class="readout-val">${readoutData.tokens.val}</span>
              ${renderDotMatrix(readoutData.tokens.grid)}
              <span class="readout-delta">${readoutData.tokens.delta}</span>
            </div>
            <div class="readout-item" title="${readoutData.cost.sub}">
              <span class="readout-label">Cost ≈</span>
              <span class="readout-val">${readoutData.cost.val}</span>
              ${renderDotMatrix(readoutData.cost.grid)}
              <span class="readout-delta">${readoutData.cost.delta}</span>
            </div>
            <div class="readout-item" title="${readoutData.cached.sub}">
              <span class="readout-label">Cached</span>
              <span class="readout-val">${readoutData.cached.val}</span>
              ${renderDotMatrix(readoutData.cached.grid)}
              <span class="readout-delta">${readoutData.cached.delta}</span>
            </div>
          </div>
          <div style="font-size:11px;color:var(--ink-muted);display:flex;justify-content:space-between">
            <span>${readoutData.cost.sub}</span>
            <span>${metrics.computersList.length} computer${metrics.computersList.length === 1 ? '' : 's'} combined</span>
          </div>
        </div>

        <!-- Col 2: Needs Attention (Exact item count, no prototype leak button) -->
        <div class="panel-card">
          <div class="panel-head">
            <span class="panel-title">Needs attention</span>
            <span style="font-size:11px;font-weight:600;color:var(--amber)">${metrics.attentionItems.length} open</span>
          </div>
          <div class="attention-list">
            ${metrics.attentionItems.length ? metrics.attentionItems.map((item) => `<div class="att-item" tabindex="0" data-action="click-attention" data-account="${item.accountId || ''}">
              <div>
                <div class="att-title">
                  <i class="pc-dot" style="background:${item.severity === 'danger' ? 'var(--coral)' : item.severity === 'info' ? 'var(--teal)' : 'var(--amber)'}"></i>
                  <span>${item.title}</span>
                </div>
                <div class="att-reason">${item.reason}</div>
              </div>
              <span class="att-when">${item.when}</span>
            </div>`).join('') : `<div style="font-size:12px;color:var(--ink-muted);padding:10px 0">Nothing needs attention right now.</div>`}
          </div>
        </div>

        <!-- Col 3: Activity, 53 Weeks (Crisp cells + 53-week computer totals) -->
        <div class="panel-card">
          <div class="panel-head">
            <span class="panel-title">Activity, 53 weeks</span>
            <div class="chip-group" role="group" aria-label="Year clock computer switch">
              <button class="filter-chip ${state.yearClockPc === 'all' ? 'active' : ''}" style="padding:2px 6px;font-size:11px" data-action="set-clock-pc" data-pc="all">All</button>
              ${D.COMPUTERS.map((c) => `<button class="filter-chip ${state.yearClockPc === c.id ? 'active' : ''}" style="padding:2px 6px;font-size:11px" data-action="set-clock-pc" data-pc="${c.id}" title="${c.name}"><i class="pc-dot" style="background:${c.col}"></i>${c.name.slice(0, 7)}</button>`).join('')}
            </div>
          </div>
          <div class="year-clock-wrap">
            ${renderYearClockSvg(state.yearClockPc, metrics)}
            <div style="display:flex;flex-direction:column;gap:5px;font-size:11px;color:var(--ink-secondary)">
              <div><b>Monday inside</b> · daily tokens</div>
              <div style="color:var(--ink-muted)">Busiest: <b style="color:var(--ink-primary)">${metrics.peakDayLabel} (${metrics.peakDayVal})</b></div>
              <div style="color:var(--ink-muted)">53-wk total: <b style="color:var(--teal)">${metrics.totalYearVal}</b></div>
              <div style="display:flex;flex-direction:column;gap:2px;margin-top:2px;font-size:11px;color:var(--ink-muted)">
                ${D.COMPUTERS.map((c) => `<span><i class="pc-dot" style="background:${c.col};margin-right:5px"></i>${c.name}: ${D.fmtTokens(c.tokens53w)} (53w)</span>`).join('')}
              </div>
            </div>
          </div>
        </div>
      </section>

      <!-- ROW 4: LATEST REPLIES & REQUEST LOG SWITCH (No Speed/tokens-per-second column on Overview per Rule 2) -->
      <section class="panel-card" aria-label="Latest replies">
        <div class="panel-head">
          <div style="display:flex;align-items:center;gap:10px">
            <span class="panel-title">Latest replies</span>
            <span style="font-size:11.5px;color:var(--ink-muted)">${metrics.repliesTodayCount.toLocaleString('en-US')} today · first reply is approximate (≈)</span>
          </div>
          <button class="filter-chip active" style="color:var(--teal);border-color:var(--teal-border)" data-action="open-request-log">
            Request log (full table) ↗
          </button>
        </div>
        <table class="replies-table">
          <thead>
            <tr>
              <th>Time</th>
              <th>Account</th>
              <th>Model</th>
              <th>Computer</th>
              <th class="num">Tokens</th>
              <th class="num">First reply</th>
            </tr>
          </thead>
          <tbody>
            ${(metrics.isFresh ? [] : D.LATEST_REPLIES).slice(0, 5).map((r) => {
              const pc = D.COMPUTERS.find((c) => c.id === r.computerId) || D.COMPUTERS[0];
              return `<tr>
                <td>${r.time} <span style="color:var(--ink-muted)">(${r.ago})</span></td>
                <td><span style="display:inline-flex;align-items:center;gap:6px">${D.LOGOS[r.tool]}<b style="color:var(--ink-primary)">${r.account}</b></span></td>
                <td>${r.model}</td>
                <td><span style="display:inline-flex;align-items:center;gap:5px"><i class="pc-dot" style="background:${pc.col}"></i>${r.computer}</span></td>
                <td class="num">${r.tokens}</td>
                <td class="num">${r.firstReply}</td>
              </tr>`;
            }).join('')}
          </tbody>
        </table>
      </section>
    </div>`;
  }

  function renderAccountsPage(state, accounts, metrics) {
    const search = (state.searchQuery || '').trim().toLowerCase();
    const filtered = accounts.filter((a) => {
      if (state.acctToolFilter !== 'all' && a.tool !== state.acctToolFilter) return false;
      if (search && !a.name.toLowerCase().includes(search) && !(a.email || '').toLowerCase().includes(search)) return false;
      return true;
    });

    filtered.sort((a,b) => {
      const room = (x) => x.measured ? (x.windows.find((w)=>w.kind==='week') || x.windows[0])?.left ?? -1 : -1;
      return room(b)-room(a);
    });

    const bestAcct = metrics.bestRoomAcct;
    const bestW5 = bestAcct && bestAcct.windows ? bestAcct.windows.find((w) => w.kind === '5h') : null;
    const bestWk = bestAcct && bestAcct.windows ? bestAcct.windows.find((w) => w.kind === 'week') : null;
    const toolsUsedCount = new Set(accounts.map((a) => a.tool)).size;

    const daysCount = state.acctRange === '30d' ? 30 : state.acctRange === '14d' ? 14 : 7;
    const chartDays = (metrics.isFresh ? [] : D.DAILY_STACK_SERIES).slice(-daysCount);
    const maxDayM = Math.max(100, ...chartDays.map((d) => d.totalM));
    const maxModelTok = Math.max(1, ...(metrics.modelsList || []).map((m) => m.tokens));

    const toolGroups = D.TOOLS.map((t) => ({
      tool: t,
      items: filtered.filter((a) => a.tool === t.id)
    })).filter((g) => g.items.length > 0);

    return `<div class="page-body">
      <section class="accounts-top-bar">
        <div class="accounts-finding">
          <h1>${bestAcct ? `<span style="color:var(--teal);white-space:nowrap">${D.esc(bestAcct.name)}</span> has the most room: ${bestW5 ? bestW5.used + '% of its 5-hour limit and ' : ''}${bestWk ? bestWk.used + '% of its week used.' : 'ready to start.'}` : accounts.length ? 'Limits have not been read yet.' : 'No accounts configured yet.'}</h1>
          <p>${accounts.length} accounts across ${toolsUsedCount} tools, each in its own folder with its own login · sorted by room left · usage at API list prices (≈).</p>
        </div>
        <div class="accounts-actions">
          <div class="chip-group" role="group" aria-label="Accounts date range">
            ${['7d', '14d', '30d'].map((r) => `<button class="filter-chip ${state.acctRange === r ? 'active' : ''}" data-action="set-acct-range" data-range="${r}">${r}</button>`).join('')}
          </div>
          <button class="btn-secondary" data-action="export-csv">Export CSV</button>
          <button class="btn-secondary" data-action="open-start-folder" data-account="${bestAcct ? bestAcct.id : ''}">Start in a folder…</button>
          <button class="btn-primary" data-action="open-add-account">+ Add account</button>
        </div>
      </section>

      <!-- Foldable Daily Chart & By-Model Breakdown -->
      <div style="display:grid;grid-template-columns:${state.foldModels ? '1.35fr 1fr' : '1fr'};gap:10px">
        <section class="panel-card">
          <div class="panel-head">
            <div style="display:flex;align-items:center;gap:10px;flex-wrap:wrap">
              <button class="panel-title" data-action="toggle-fold-daily">
                ${state.foldDaily ? '▾' : '▸'} Tokens per day, by account (${state.acctRange})
              </button>
              <div style="display:flex;gap:8px;flex-wrap:wrap;font-size:11px;color:var(--ink-muted)">
                ${(!metrics.isFresh && D.DAILY_STACK_SERIES[0] ? D.DAILY_STACK_SERIES[0].parts : []).map((p) => `<span><i class="pc-dot" style="background:${p.col}"></i> ${p.id}</span>`).join('')}
              </div>
            </div>
            <button class="filter-chip ${state.foldModels ? 'active' : ''}" data-action="toggle-fold-models">
              ${state.foldModels ? '▾ Hide By model' : '▸ By model'}
            </button>
          </div>
          ${state.foldDaily ? `<div class="stacked-chart-box" style="height:78px">
            ${chartDays.map((d) => {
              const totalH = Math.max(8, Math.round((d.totalM / maxDayM) * 58));
              return `<div class="stack-col" title="${d.label}: ${(d.totalM / 1e3).toFixed(2)}B tokens">
                <div class="stack-bars" style="height:${totalH}px">
                  ${d.parts.map((p) => {
                    const segH = Math.max(1, Math.round((p.valM / d.totalM) * totalH));
                    return `<div class="stack-seg" style="height:${segH}px;background:${p.col}"></div>`;
                  }).join('')}
                </div>
                <span class="stack-lbl">${d.label}</span>
              </div>`;
            }).join('')}
          </div>` : ''}
        </section>

        ${state.foldModels ? `<section class="panel-card">
          <div class="panel-head">
            <span class="panel-title">By model (${state.acctRange})</span>
            <span style="font-size:11px;color:var(--ink-muted)">API list prices ≈</span>
          </div>
          <div style="display:flex;flex-direction:column;gap:6px;font-size:11.5px">
            ${metrics.modelsList.map((m) => {
              const wPct = Math.max(4, Math.round((m.tokens / maxModelTok) * 100));
              return `<div class="win-bar-line"><span>${m.model}</span><div class="win-track"><div class="win-fill" style="width:${wPct}%"></div></div><span>${D.fmtTokens(m.tokens)}</span><span style="text-align:right">${D.fmtMoney(m.cost)}</span></div>`;
            }).join('')}
          </div>
        </section>` : ''}
      </div>

      <!-- Tool Filter Chips -->
      <div class="horizon-filterbar" style="border:1px solid var(--line-default);border-radius:8px">
        <div class="chip-group">
          <button class="filter-chip ${state.acctToolFilter === 'all' ? 'active' : ''}" data-action="set-acct-tool" data-tool="all">All (${accounts.length})</button>
          ${D.TOOLS.map((t) => `<button class="filter-chip ${state.acctToolFilter === t.id ? 'active' : ''}" data-action="set-acct-tool" data-tool="${t.id}">${D.LOGOS[t.id]}<span>${t.name}</span></button>`).join('')}
        </div>
        <span style="font-size:11.5px;color:var(--ink-muted)">Sorted by room left · click or press Enter on any row for account detail</span>
      </div>

      <!-- Foldable Tool Groups & Account Rows -->
      ${toolGroups.map((g) => {
        const groupTok = g.items.reduce((s, a) => s + (a.tokens7d || 0), 0);
        const groupCost = g.items.reduce((s, a) => s + (a.cost7d || 0), 0);
        const groupOpen = g.items.reduce((s, a) => s + (a.openSessions || 0), 0);
        const earliestResetMs = Math.min(...g.items.flatMap((a) => (a.windows || []).map((w) => w.resetMs)).concat([Infinity]));
        const nextResetStr = isFinite(earliestResetMs) ? D.fmtDuration(earliestResetMs) : '—';
        const isCollapsed = !!state.collapsedTools[g.tool.id];
        return `<section class="tool-group-card">
          <div class="tool-group-header" tabindex="0" data-action="toggle-tool-group" data-tool="${g.tool.id}">
            <div style="display:flex;align-items:center;gap:8px;font-weight:700;font-size:13px">
              <span>${isCollapsed ? '▸' : '▾'}</span>
              ${D.LOGOS[g.tool.id]}
              <span>${g.tool.name}</span>
              <span style="color:var(--ink-muted);font-weight:500">${g.items.length}</span>
            </div>
            <div style="display:flex;align-items:center;gap:12px;font-size:11.5px;color:var(--ink-muted)">
              <span>${g.items.filter((a) => !a.needsLogin).length} logged in</span>
              <span style="color:var(--teal)">${groupOpen} consoles open</span>
              <span>next reset ${nextResetStr}</span>
              <span>${D.fmtTokens(groupTok)} tokens</span>
              <span>${D.fmtMoney(groupCost)}</span>
            </div>
          </div>
          ${!isCollapsed ? g.items.map((acct) => `
            <div class="acct-row" tabindex="0" data-action="open-account-detail" data-account="${acct.id}">
              <div style="display:flex;align-items:center;gap:10px;min-width:0">
                ${renderRingSvg(acct, state.showUsedPct)}
                <div style="min-width:0">
                  <div style="display:flex;align-items:center;gap:6px">
                    <b style="font-size:13px;color:var(--ink-primary);white-space:nowrap">${D.esc(acct.name)}</b>
                    ${acct.isMain ? `<span class="plan-pill" title="Main tool profile (${D.esc(acct.folder)}) — can be hidden, not deleted">main</span>` : ''}
                  </div>
                  <div style="font-size:11px;color:var(--ink-muted);overflow:hidden;text-overflow:ellipsis">${D.esc(acct.email)}</div>
                  <div style="display:flex;align-items:center;gap:6px;margin-top:3px">
                    <span class="plan-pill" style="color:var(--teal);border-color:var(--teal-border)">${D.esc(acct.plan)}</span>
                    <span style="font-size:11px;color:var(--ink-muted)">Added ${acct.added}</span>
                  </div>
                </div>
              </div>

              <div class="acct-windows-col">
                ${acct.windows && acct.windows.length ? acct.windows.map((w) => `
                  <div class="win-bar-line">
                    <span style="color:var(--ink-muted)">${w.label}</span>
                    <div class="win-track"><div class="win-fill" style="width:${w.left}%;background:${w.left <= 28 ? 'var(--amber)' : 'var(--teal)'}"></div></div>
                    <span style="font-weight:600">${w.left}% left</span>
                    <span style="color:var(--ink-muted)">↻ ${D.fmtDuration(w.resetMs)}</span>
                  </div>
                `).join('') : `<div style="font-size:11.5px;color:var(--ink-muted)">${!['claude','codex'].includes(acct.tool) || acct.kind !== 'subscription' ? 'No plan limits · usage only' : '— plan limits not read'}${acct.needsLogin ? ' · login expired' : ''}</div>`}
                <div style="font-size:11px;margin-top:2px;color:${acct.adviceTone === 'danger' || acct.adviceTone === 'warn' ? 'var(--amber)' : 'var(--teal)'}">
                  ${D.esc(acct.advice)}
                </div>
              </div>

              <div>
                <div class="spark-bars" aria-label="7-day token sparkline">
                  ${(acct.spark7d || [0,0,0,0,0,0,0]).map((v) => `<i style="height:${Math.max(2, Math.round(v * 24))}px"></i>`).join('')}
                </div>
                <div style="font-size:11px;color:var(--ink-muted);margin-top:2px">7-day pace</div>
              </div>

              <div style="text-align:right">
                <div style="font-weight:700;font-size:13px">${D.fmtTokens(acct.tokens7d)} <span style="font-weight:400;font-size:11px;color:var(--ink-muted)">tokens</span></div>
                <div style="font-size:12px;color:var(--ink-secondary)">${D.fmtMoney(acct.cost7d)} <span style="font-size:11px;color:var(--ink-muted)">list ≈</span></div>
                ${acct.openSessions > 0 ? `<div style="font-size:11px;color:var(--teal);margin-top:2px">● ${acct.openSessions} open</div>` : ''}
              </div>

              <div style="display:flex;align-items:center;justify-content:flex-end;gap:5px">
                <span style="font-size:11px;color:var(--ink-muted)">${acct.readAgoSec != null ? `${Math.max(1, Math.round(acct.readAgoSec / 60))}m ago` : 'never'}</span>
                <button class="icon-btn" title="Refresh limits live" data-action="refresh-single-account" data-account="${acct.id}">↻</button>
                <button class="btn-lane-start" style="opacity:1;pointer-events:auto" data-action="start-account" data-account="${acct.id}">Start</button>
                <button class="icon-btn" title="Account actions (folder, rename, sync, remove)" data-action="open-account-menu" data-account="${acct.id}">⋯</button>
              </div>
            </div>
          `).join('') : ''}
        </section>`;
      }).join('')}
    </div>`;
  }

  function renderFirstRunOrEmpty(state, accounts) {
    if (state.preset === 'empty' && accounts.length === 0) {
      return `<div class="page-body">
        <section class="panel-card" style="padding:28px;max-width:760px;margin:20px auto;text-align:center;align-items:center;gap:14px">
          <div style="width:44px;height:52px">${D.LOGOS.harness}</div>
          <h1 style="font-size:19px;font-weight:700">No AI coding tool found on DEVELOPMENTPC</h1>
          <div class="tool-pick-grid" style="width:100%;margin-top:4px">
            ${D.TOOLS.map((t) => `<div class="tool-pick-card">
              <div style="display:flex;align-items:center;justify-content:space-between">
                <span style="display:inline-flex;align-items:center;gap:6px;font-weight:700">${D.LOGOS[t.id]} ${t.name}</span>
                <span class="plan-pill">Not installed</span>
              </div>
              <div style="font-size:11px;color:var(--ink-muted)">Supports: ${Object.keys(t.kinds).filter((k) => t.kinds[k].supported).map((k) => k === 'apikey' ? 'API key' : k.charAt(0).toUpperCase() + k.slice(1)).join(' · ')}</div>
              <button class="btn-secondary" style="margin-top:4px;justify-content:center" data-action="open-add-account-tool" data-tool="${t.id}">Prepare ${t.name} account</button>
            </div>`).join('')}
          </div>
          <div style="display:flex;gap:10px;margin-top:6px">
            <button class="btn-primary" data-action="open-add-account">+ Add first account</button>
          </div>
        </section>
      </div>`;
    }

    return `<div class="page-body">
      <section class="panel-card" style="border-color:var(--teal-border);padding:20px;gap:14px">
        <div class="eyebrow-row">
          <span class="eyebrow teal">Profiles on DEVELOPMENTPC · ${accounts.length} profiles ready</span>
          <span style="font-size:11px;color:var(--ink-muted)">Limits not read (—) until first sign-in or refresh</span>
        </div>
        <div style="display:grid;grid-template-columns:1.25fr 1fr;gap:16px;align-items:start">
          <div style="display:flex;flex-direction:column;gap:10px">
            <h2 style="font-size:17px;font-weight:700">Limits have not been read yet.</h2>
            <div class="detected-profiles" style="display:flex;flex-direction:column;gap:6px">
              ${accounts.map((a) => `<div class="live-mini-row" style="padding:8px 10px">
                <div class="live-mini-left">
                  ${D.LOGOS[a.tool]}
                  <b>${D.esc(a.name)}</b>
                  <span class="plan-pill">${a.isMain ? 'main profile' : a.plan}</span>
                  <span class="mono" style="color:var(--ink-muted);font-size:11px">${D.esc(a.folder)}</span>
                  <span style="color:var(--ink-muted);font-size:11px">· ${['claude','codex'].includes(a.tool) ? '— limits not read' : 'Usage only · no plan limits'}</span>
                </div>
                <button class="btn-lane-start" style="opacity:1;pointer-events:auto" data-action="start-account" data-account="${a.id}">▶ Start</button>
              </div>`).join('')}
            </div>
            <div style="display:flex;gap:8px;margin-top:4px">
              <button class="btn-primary" data-action="open-add-account">+ Add another account</button>
            </div>
          </div>

          <div class="panel-card" style="background:var(--bg-card)">
            <div class="panel-title">Sync limits &amp; live sessions across your computers?</div>
            <div style="display:flex;flex-direction:column;gap:7px;margin-top:4px">
              <label class="candidate-pill ${state.syncChoice === 'none' ? 'active' : ''}" tabindex="0" style="cursor:pointer" data-action="set-sync-choice" data-sync="none">
                <div>
                  <b>Not now — this computer only</b>
                  <div style="color:var(--ink-muted)">Enable multi-computer sync anytime in Settings › Computers</div>
                </div>
                <span>Local</span>
              </label>
              <label class="candidate-pill ${state.syncChoice === 'folder' ? 'active' : ''}" tabindex="0" style="cursor:pointer" data-action="set-sync-choice" data-sync="folder">
                <div>
                  <b>Shared folder (OneDrive / Dropbox / iCloud / SMB)</b>
                  <div style="color:var(--ink-muted)">Syncs limits and sessions across your computers</div>
                </div>
                <span style="color:var(--teal)">Folder</span>
              </label>
              <label class="candidate-pill ${state.syncChoice === 'hub' ? 'active' : ''}" tabindex="0" style="cursor:pointer" data-action="set-sync-choice" data-sync="hub">
                <div>
                  <b>Self-hosted Harness Hub URL</b>
                  <div style="color:var(--ink-muted)">Real-time WebSocket relay over Tailscale or LAN</div>
                </div>
                <span>wss://</span>
              </label>
            </div>
          </div>
        </div>
      </section>
    </div>`;
  }

  window.HM_VIEWS = {
    linearPct,
    renderRingSvg,
    renderDotMatrix,
    renderYearClockSvg,
    renderCapacityColumn,
    renderLinearRunwayTrack,
    renderOverview,
    renderAccountsPage,
    renderFirstRunOrEmpty
  };
})();

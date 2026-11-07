// Designer C (c-opencode) — Chrono-Runway mock data & official SVG assets
// Classic script so index.html opens directly via file:// with no server or build step.
(function () {
  const MIN = 60e3;
  const H = 3600e3;
  const DAY = 24 * H;
  // Reference "NOW": Fri 6 Nov 2026, 15:42 local time
  const NOW = new Date(2026, 10, 6, 15, 42, 0).getTime();

  // Official inline SVGs (from docs/logo.svg and app/ui/icons/*.svg)
  const LOGOS = {
    harness: `<svg xmlns="http://www.w3.org/2000/svg" viewBox="60 10 410 500" role="img" aria-label="Harness Manager"><path d="M84 259 L84 115 C84 90 96 77 121 63 L179 31 C194 23 207 25 207 44 L207 211 C173 229 122 255 84 280 Z" fill="url(#hm-lu)"/><path d="M188 292 C231 302 284 329 333 356 L333 425 C286 402 236 376 184 347 L153 330 Z" fill="url(#hm-bc)"/><path d="M84 342 L84 381 C84 402 96 417 113 428 L176 473 C196 487 207 482 207 465 L207 305 C170 320 119 340 84 342 Z" fill="url(#hm-ll)"/><path d="M333 41 C333 24 342 20 358 28 L418 62 C439 74 447 90 447 110 L447 192 C408 218 372 236 333 251 Z" fill="url(#hm-ru)"/><path d="M333 244 L447 190 L447 287 L333 344 Z" fill="url(#hm-rm)"/><path d="M333 340 L447 283 L447 413 C447 435 437 450 416 464 L365 494 C347 506 333 496 333 479 Z" fill="url(#hm-rl)"/><path d="M84 270 C84 242 106 229 149 208 L333 121 L333 244 L151 332 C111 353 91 368 84 393 Z" fill="url(#hm-fc)"/></svg>`,
    claude: `<svg class="tool-svg" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Claude Code"><path clip-rule="evenodd" d="M20.998 10.949H24v3.102h-3v3.028h-1.487V20H18v-2.921h-1.487V20H15v-2.921H9V20H7.488v-2.921H6V20H4.487v-2.921H3V14.05H0V10.95h3V5h17.998v5.949zM6 10.949h1.488V8.102H6v2.847zm10.51 0H18V8.102h-1.49v2.847z" fill="#D97757" fill-rule="evenodd"></path></svg>`,
    codex: `<svg class="tool-svg" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Codex"><path d="M19.503 0H4.496A4.496 4.496 0 000 4.496v15.007A4.496 4.496 0 004.496 24h15.007A4.496 4.496 0 0024 19.503V4.496A4.496 4.496 0 0019.503 0z" fill="#fff"></path><path d="M9.064 3.344a4.578 4.578 0 012.285-.312c1 .115 1.891.54 2.673 1.275.01.01.024.017.037.021a.09.09 0 00.043 0 4.55 4.55 0 013.046.275l.047.022.116.057a4.581 4.581 0 012.188 2.399c.209.51.313 1.041.315 1.595a4.24 4.24 0 01-.134 1.223.123.123 0 00.03.115c.594.607.988 1.33 1.183 2.17.289 1.425-.007 2.71-.887 3.854l-.136.166a4.548 4.548 0 01-2.201 1.388.123.123 0 00-.081.076c-.191.551-.383 1.023-.74 1.494-.9 1.187-2.222 1.846-3.711 1.838-1.187-.006-2.239-.44-3.157-1.302a.107.107 0 00-.105-.024c-.388.125-.78.143-1.204.138a4.441 4.441 0 01-1.945-.466 4.544 4.544 0 01-1.61-1.335c-.152-.202-.303-.392-.414-.617a5.81 5.81 0 01-.37-.961 4.582 4.582 0 01-.014-2.298.124.124 0 00.006-.056.085.085 0 00-.027-.048 4.467 4.467 0 01-1.034-1.651 3.896 3.896 0 01-.251-1.192 5.189 5.189 0 01.141-1.6c.337-1.112.982-1.985 1.933-2.618.212-.141.413-.251.601-.33.215-.089.43-.164.646-.227a.098.098 0 00.065-.066 4.51 4.51 0 01.829-1.615 4.535 4.535 0 011.837-1.388zm3.482 10.565a.637.637 0 000 1.272h3.636a.637.637 0 100-1.272h-3.636zM8.462 9.23a.637.637 0 00-1.106.631l1.272 2.224-1.266 2.136a.636.636 0 101.095.649l1.454-2.455a.636.636 0 00.005-.64L8.462 9.23z" fill="url(#lobe-icons-codex-_R_0_)"></path></svg>`,
    opencode: `<svg class="tool-svg" fill="currentColor" fill-rule="evenodd" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="OpenCode"><path d="M16 6H8v12h8V6zm4 16H4V2h16v20z"></path></svg>`,
    agy: `<svg class="tool-svg" viewBox="-1.5 -1.5 27 27" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Antigravity"><g mask="url(#lobe-icons-antigravity-0-_R_0_)"><g filter="url(#lobe-icons-antigravity-1-_R_0_)"><path d="M-1.018-3.992c-.408 3.591 2.686 6.89 6.91 7.37 4.225.48 7.98-2.043 8.387-5.633.408-3.59-2.686-6.89-6.91-7.37-4.225-.479-7.98 2.043-8.387 5.633z" fill="#FFE432"></path></g><g filter="url(#lobe-icons-antigravity-2-_R_0_)"><path d="M15.269 7.747c1.058 4.557 5.691 7.374 10.348 6.293 4.657-1.082 7.575-5.653 6.516-10.21-1.058-4.556-5.691-7.374-10.348-6.292-4.657 1.082-7.575 5.653-6.516 10.21z" fill="#FC413D"></path></g><g filter="url(#lobe-icons-antigravity-3-_R_0_)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C"></path></g><g filter="url(#lobe-icons-antigravity-4-_R_0_)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C"></path></g><g filter="url(#lobe-icons-antigravity-5-_R_0_)"><path d="M-7.608 14.703c3.352 3.424 9.126 3.208 12.896-.483 3.77-3.69 4.108-9.459.756-12.883C2.69-2.087-3.083-1.871-6.853 1.82c-3.77 3.69-4.108 9.458-.755 12.883z" fill="#00B95C"></path></g><g filter="url(#lobe-icons-antigravity-6-_R_0_)"><path d="M9.932 27.617c1.04 4.482 5.384 7.303 9.7 6.3 4.316-1.002 6.971-5.448 5.93-9.93-1.04-4.483-5.384-7.304-9.7-6.301-4.316 1.002-6.971 5.448-5.93 9.93z" fill="#3186FF"></path></g><g filter="url(#lobe-icons-antigravity-7-_R_0_)"><path d="M2.572-8.185C.392-3.329 2.778 2.472 7.9 4.771c5.122 2.3 11.042.227 13.222-4.63 2.18-4.855-.205-10.656-5.327-12.955-5.122-2.3-11.042-.227-13.222 4.63z" fill="#FBBC04"></path></g><g filter="url(#lobe-icons-antigravity-8-_R_0_)"><path d="M-3.267 38.686c-5.277-2.072 3.742-19.117 5.984-24.83 2.243-5.712 8.34-8.664 13.616-6.592 5.278 2.071 11.533 13.482 9.29 19.195-2.242 5.713-23.613 14.298-28.89 12.227z" fill="#3186FF"></path></g><g filter="url(#lobe-icons-antigravity-9-_R_0_)"><path d="M28.71 17.471c-1.413 1.649-5.1.808-8.236-1.878-3.135-2.687-4.531-6.201-3.118-7.85 1.412-1.649 5.1-.808 8.235 1.878s4.532 6.2 3.119 7.85z" fill="#749BFF"></path></g><g filter="url(#lobe-icons-antigravity-10-_R_0_)"><path d="M18.163 9.077c5.81 3.93 12.502 4.19 14.946.577 2.443-3.612-.287-9.727-6.098-13.658-5.81-3.931-12.502-4.19-14.946-.577-2.443 3.612.287 9.727 6.098 13.658z" fill="#FC413D"></path></g><g filter="url(#lobe-icons-antigravity-11-_R_0_)"><path d="M-.915 2.684c-1.44 3.473-.97 6.967 1.05 7.804 2.02.837 4.824-1.3 6.264-4.772 1.44-3.473.97-6.967-1.05-7.804-2.02-.837-4.824 1.3-6.264 4.772z" fill="#FFEE48"></path></g></g></svg>`,
    pi: `<svg class="tool-svg" viewBox="-1.5 -1.5 27 27" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Pi"><rect x="0" y="0" width="24" height="24" rx="5" fill="#1F2937"></rect><path d="M5.5 7.5h13M8.5 7.5v10M15.5 7.5v10" fill="none" stroke="#F5F5F4" stroke-width="2.2" stroke-linecap="round"></path></svg>`,
    omp: `<svg class="tool-svg" viewBox="-1.5 -1.5 27 27" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Oh My Pi"><rect x="0" y="0" width="24" height="24" rx="5" fill="#7C3AED"></rect><path d="M5.5 7.5h13M8.5 7.5v10M15.5 7.5v10" fill="none" stroke="#FFFFFF" stroke-width="2.2" stroke-linecap="round"></path><circle cx="19" cy="5" r="2.2" fill="#FDE68A"></circle></svg>`
  };

  // Computers (exact names from brief.md)
  const COMPUTERS = [
    {
      id: 'developmentpc',
      name: 'DEVELOPMENTPC',
      short: 'DEVELOPMENTPC',
      os: 'Windows 11 Pro · 64GB',
      me: true,
      col: '#3FD6CF',
      seenMsAgo: 0,
      liveCount: 12,
      tokensToday: 1.48e9,
      tokens53w: 88.5e9
    },
    {
      id: 'samymsi',
      name: 'SAMYMSI',
      short: 'SAMYMSI',
      os: 'Windows 11 · RTX 4090',
      me: false,
      col: '#E5A93C',
      seenMsAgo: 38e3,
      liveCount: 4,
      tokensToday: 0.69e9,
      tokens53w: 37.1e9
    },
    {
      id: 'macbook',
      name: 'Samys-MacBook-Pro',
      short: 'Samys-MacBook-Pro',
      os: 'macOS Sequoia · M4 Max',
      me: false,
      col: '#9E8CFC',
      seenMsAgo: 84e3,
      liveCount: 2,
      tokensToday: 0.24e9,
      tokens53w: 17.2e9
    }
  ];

  // All 6 tools and their explicit account-kind compatibility matrix for Add Account
  const TOOLS = [
    {
      id: 'claude',
      name: 'Claude Code',
      ver: '2.4.1',
      installed: true,
      updateAvailable: null,
      folderPrefix: '~\\.claude-',
      mainFolder: '~\\.claude',
      provider: 'Anthropic Direct',
      firstReplySec: 1.1,
      kinds: {
        subscription: {
          supported: true,
          plans: 'Pro, Max 5x, Max 20x, Team or Enterprise',
          loginFlow: 'Browser OAuth inside Harness Manager (paste verification code)',
          windowsDesc: '5-hour rolling window + 7-day weekly window + per-model weekly window'
        },
        apikey: {
          supported: true,
          envVar: 'ANTHROPIC_API_KEY',
          endpointDefault: 'https://api.anthropic.com',
          defaultModel: 'claude-sonnet-4-5'
        },
        routed: {
          supported: true,
          routerNote: 'Points ANTHROPIC_BASE_URL at local Harness Router (127.0.0.1:8787)'
        }
      },
      syncedItems: 'CLAUDE.md, rules, skills (255), plugins and MCP servers (11)',
      isolatedItems: 'OAuth login token, session history, project pins and rate-limit counters'
    },
    {
      id: 'codex',
      name: 'Codex',
      ver: '0.167.0',
      installed: true,
      updateAvailable: '0.168.0',
      folderPrefix: '~\\.codex-',
      mainFolder: '~\\.codex',
      provider: 'OpenAI Direct',
      firstReplySec: 0.9,
      kinds: {
        subscription: {
          supported: true,
          plans: 'ChatGPT Plus, Pro, Team or Enterprise',
          loginFlow: 'Opens terminal window for `codex login` OAuth handshake',
          windowsDesc: '5-hour rolling window + 7-day weekly window'
        },
        apikey: {
          supported: true,
          envVar: 'OPENAI_API_KEY',
          endpointDefault: 'https://api.openai.com/v1',
          defaultModel: 'gpt-5.3-codex'
        },
        routed: {
          supported: true,
          routerNote: 'Routes OpenAI Responses API calls via local Harness Router (:8787)'
        }
      },
      syncedItems: 'AGENTS.md, global instructions, skills and MCP server bindings',
      isolatedItems: 'OpenAI session credentials, local rollout logs and usage ledger'
    },
    {
      id: 'opencode',
      name: 'OpenCode',
      ver: '1.3.2',
      installed: true,
      updateAvailable: null,
      folderPrefix: '~\\.opencode-',
      mainFolder: '~\\.config\\opencode',
      provider: 'OpenCode',
      firstReplySec: 0.6,
      kinds: {
        subscription: {
          supported: true,
          plans: 'OpenCode Go or provider OAuth',
          loginFlow: 'Terminal sign-in (`opencode auth login`)',
          windowsDesc: 'Provider-reported window or daily spend cap'
        },
        apikey: {
          supported: false,
          reason: 'Not supported by OpenCode in Harness Manager (use Subscription or Routed).'
        },
        routed: {
          supported: true,
          routerNote: 'First-class routed profile via local Harness Router (:8787)'
        }
      },
      syncedItems: 'opencode.jsonc, AGENTS.md, custom commands and MCP servers',
      isolatedItems: 'Provider credentials, route target, session state and spend cap'
    },
    {
      id: 'agy',
      name: 'Antigravity',
      ver: '1.8.0',
      installed: true,
      updateAvailable: null,
      folderPrefix: '~\\.antigravity-',
      mainFolder: '~\\.antigravity',
      provider: 'Antigravity',
      firstReplySec: null,
      kinds: {
        subscription: {
          supported: true,
          plans: 'Google Workspace AI Ultra / Individual Pro',
          loginFlow: 'Terminal browser callback (`agy auth login`)',
          windowsDesc: '5-hour + weekly quota windows (requires active Google session)'
        },
        apikey: {
          supported: false,
          reason: 'Antigravity supports Google OAuth Subscription sign-in only.'
        },
        routed: {
          supported: false,
          reason: 'Antigravity uses pinned gRPC endpoints and does not support local HTTP proxy routing.'
        }
      },
      syncedItems: 'Global rules, MCP servers and workspace templates',
      isolatedItems: 'Google OAuth refresh token and workspace quota state'
    },
    {
      id: 'pi',
      name: 'Pi',
      ver: '0.41.0',
      installed: true,
      updateAvailable: null,
      folderPrefix: '~\\.pi-',
      mainFolder: '~\\.pi',
      provider: 'Pi',
      firstReplySec: 1.3,
      kinds: {
        subscription: {
          supported: true,
          plans: 'Pi Pro / Provider OAuth',
          loginFlow: 'Terminal sign-in (`pi login`)',
          windowsDesc: '5-hour + weekly quota windows'
        },
        apikey: {
          supported: true,
          envVar: 'PI_API_KEY',
          endpointDefault: 'https://openrouter.ai/api/v1',
          defaultModel: 'anthropic/claude-sonnet-4-5'
        },
        routed: {
          supported: true,
          routerNote: 'Routes through local Harness Router (:8787) with unified cost tracking'
        }
      },
      syncedItems: 'Skills, system prompts and tool definitions',
      isolatedItems: 'Login/API key, route binding and transcript history'
    },
    {
      id: 'omp',
      name: 'Oh My Pi',
      ver: '0.19.4',
      installed: false,
      updateAvailable: null,
      folderPrefix: '~\\.omp-',
      mainFolder: '~\\.omp',
      provider: 'Oh My Pi',
      firstReplySec: 1.2,
      kinds: {
        subscription: {
          supported: true,
          plans: 'Oh My Pi Subscription',
          loginFlow: 'Terminal sign-in (`omp login`)',
          windowsDesc: '5-hour + weekly quota windows'
        },
        apikey: {
          supported: false,
          reason: 'Not supported by Oh My Pi (use Subscription or Routed).'
        },
        routed: {
          supported: true,
          routerNote: 'Connects to local Harness Router (:8787) once Oh My Pi binary is installed'
        }
      },
      syncedItems: 'Shared Pi skills, prompts and harness hooks',
      isolatedItems: 'Profile config and per-profile ledger'
    }
  ];

  // The owner's 8 accounts (matching home-dark.png, accounts-dark.png, and brief.md)
  const BASE_ACCOUNTS = [
    {
      id: 'service-account-resal',
      name: 'service-account-resal',
      isMain: false,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Max 5x',
      email: 'service@example.me',
      folder: '~\\.claude-service-account-resal',
      added: 'Nov 4',
      readAgoSec: 110,
      measured: true,
      bestBetReason: 'Most room (98% 5h · 100% week) · 5h resets in 4h 08m',
      advice: 'Most room of any account: start here next.',
      adviceTone: 'good',
      openSessions: 1,
      computers: ['developmentpc'],
      providerId: 'anthropic',
      firstReplySec: 1.0,
      tokens7d: 2.75e6,
      cost7d: 1.61,
      spark7d: [0.05, 0.08, 0.04, 0.12, 0.09, 0.18, 0.95],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 2, left: 98, resetMs: (4 * 60 + 8) * MIN, forecastAtReset: 14, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 0, left: 100, resetMs: (4 * 24 + 7) * H, forecastAtReset: 18, outMs: null },
        { id: 'model', label: 'Week · sonnet', kind: 'model', used: 0, left: 100, resetMs: (4 * 24 + 7) * H, forecastAtReset: 16, outMs: null }
      ],
      modelsBreakdown: [
        { model: 'claude-sonnet-4-5', tokens: 2.45e6, cost: 1.38, share: 89 },
        { model: 'claude-haiku-4-5', tokens: 0.30e6, cost: 0.23, share: 11 }
      ],
      recentFolders: ['D:\\Projects\\resal\\sanduq-api', 'D:\\Projects\\abushanab-net\\HarnessManager', 'D:\\Projects\\resal\\resal-wiki']
    },
    {
      id: 'sabushanab-resal',
      name: 'sabushanab-resal',
      isMain: false,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Max 5x',
      email: 's.abushanab@resal.me',
      folder: '~\\.claude-sabushanab-resal',
      added: 'Oct 19',
      readAgoSec: 45,
      measured: true,
      expiringSoonReason: '5h window resets in 52 m with 87% still unused',
      advice: '5-hour window resets in 52 m with 87% left — use before it resets.',
      adviceTone: 'harvest',
      openSessions: 2,
      computers: ['developmentpc', 'samymsi'],
      providerId: 'anthropic',
      firstReplySec: 1.1,
      tokens7d: 184.2e6,
      cost7d: 68.40,
      spark7d: [0.32, 0.48, 0.41, 0.63, 0.52, 0.70, 0.58],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 13, left: 87, resetMs: 52 * MIN, forecastAtReset: 29, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 13, left: 87, resetMs: (3 * 24 + 19) * H, forecastAtReset: 44, outMs: null },
        { id: 'model', label: 'Week · sonnet', kind: 'model', used: 16, left: 84, resetMs: (3 * 24 + 19) * H, forecastAtReset: 49, outMs: null }
      ],
      modelsBreakdown: [
        { model: 'claude-sonnet-4-5', tokens: 152.0e6, cost: 51.20, share: 82 },
        { model: 'claude-opus-4-6', tokens: 32.2e6, cost: 17.20, share: 18 }
      ],
      recentFolders: ['D:\\Projects\\resal\\checkout-service', 'D:\\Projects\\resal\\merchant-portal']
    },
    {
      id: 'samykabu-personal',
      name: 'samykabu-personal',
      isMain: false,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Max 20x',
      email: 'samy@example.com',
      folder: '~\\.claude-samykabu-personal',
      added: 'Nov 4',
      readAgoSec: 120,
      measured: true,
      advice: '96% of week left · resets in 6 d 14 h.',
      adviceTone: 'good',
      openSessions: 1,
      computers: ['macbook'],
      providerId: 'anthropic',
      firstReplySec: 1.2,
      tokens7d: 56.85e6,
      cost7d: 41.41,
      spark7d: [0.10, 0.08, 0.14, 0.12, 0.85, 0.19, 0.22],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 6, left: 94, resetMs: (3 * 60 + 22) * MIN, forecastAtReset: 15, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 4, left: 96, resetMs: (6 * 24 + 14) * H, forecastAtReset: 28, outMs: null },
        { id: 'model', label: 'Week · sonnet', kind: 'model', used: 6, left: 94, resetMs: (6 * 24 + 14) * H, forecastAtReset: 34, outMs: null }
      ],
      modelsBreakdown: [
        { model: 'claude-sonnet-4-5', tokens: 48.1e6, cost: 31.10, share: 85 },
        { model: 'claude-opus-4-6', tokens: 8.75e6, cost: 10.31, share: 15 }
      ],
      recentFolders: ['~/Projects/abushanab-net/blog', '~/Projects/personal/synth-webgl']
    },
    {
      id: 'claude-main',
      name: 'claude-main',
      isMain: true,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Max 20x',
      email: 'samy@example.com',
      folder: '~\\.claude',
      added: 'Nov 4',
      readAgoSec: 120,
      measured: true,
      advice: 'Week runs out Thu 03:38 at this pace (fable model window Tue 08:04).',
      adviceTone: 'warn',
      openSessions: 5,
      computers: ['developmentpc', 'samymsi'],
      providerId: 'anthropic',
      firstReplySec: 1.1,
      tokens7d: 7.51e9,
      cost7d: 2795.31,
      spark7d: [0.68, 0.55, 0.92, 0.44, 0.65, 0.78, 0.96],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 58, left: 42, resetMs: (2 * 60 + 19) * MIN, forecastAtReset: 86, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 4, left: 96, resetMs: (6 * 24 + 14) * H, forecastAtReset: 100, outMs: (5 * 24 + 11) * H + 56 * MIN },
        { id: 'model', label: 'Week · fable', kind: 'model', used: 38, left: 62, resetMs: (6 * 24 + 14) * H, forecastAtReset: 100, outMs: (3 * 24 + 16) * H + 22 * MIN }
      ],
      modelsBreakdown: [
        { model: 'claude-opus-4-6', tokens: 4.92e9, cost: 2140.10, share: 65 },
        { model: 'claude-sonnet-4-5', tokens: 2.59e9, cost: 655.21, share: 35 }
      ],
      recentFolders: ['D:\\Projects\\abushanab-net\\HarnessManager', 'D:\\Projects\\resal\\core-ledger']
    },
    {
      id: 'samykabu-teksa',
      name: 'samykabu-teksa',
      isMain: false,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Max 5x',
      email: 'samy@teksa.io',
      folder: '~\\.claude-samykabu-teksa',
      added: 'Oct 12',
      readAgoSec: 180,
      measured: true,
      advice: '77% of week left · resets in 5 d 2 h (projected 68% used at reset).',
      adviceTone: 'neutral',
      openSessions: 2,
      computers: ['samymsi'],
      providerId: 'anthropic',
      firstReplySec: 1.1,
      tokens7d: 412.6e6,
      cost7d: 148.90,
      spark7d: [0.42, 0.39, 0.51, 0.48, 0.35, 0.61, 0.49],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 29, left: 71, resetMs: (2 * 60 + 45) * MIN, forecastAtReset: 54, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 23, left: 77, resetMs: (5 * 24 + 2) * H, forecastAtReset: 68, outMs: null },
        { id: 'model', label: 'Week · sonnet', kind: 'model', used: 26, left: 74, resetMs: (5 * 24 + 2) * H, forecastAtReset: 71, outMs: null }
      ],
      modelsBreakdown: [
        { model: 'claude-sonnet-4-5', tokens: 412.6e6, cost: 148.90, share: 100 }
      ],
      recentFolders: ['D:\\Work\\teksa\\edge-gateway', 'D:\\Work\\teksa\\telemetry-ui']
    },
    {
      id: 'codex-main',
      name: 'codex-main',
      isMain: true,
      tool: 'codex',
      kind: 'subscription',
      plan: 'Pro',
      email: 'samy@example.com',
      folder: '~\\.codex',
      added: 'Oct 28',
      readAgoSec: 18,
      measured: true,
      exhaustionAlert: true,
      advice: 'Will likely run out of its week Sun at about 00:56, 2 d 14 h before it resets (Tue 15:46).',
      adviceTone: 'danger',
      openSessions: 4,
      computers: ['developmentpc', 'samymsi'],
      providerId: 'openai',
      firstReplySec: 0.9,
      tokens7d: 1.48e9,
      cost7d: 412.80,
      spark7d: [0.58, 0.64, 0.79, 0.71, 0.84, 0.91, 0.98],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 48, left: 52, resetMs: (1 * 60 + 14) * MIN, forecastAtReset: 79, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 73, left: 27, resetMs: (3 * 24 + 24) * H + 4 * MIN, forecastAtReset: 100, outMs: (1 * 24 + 9) * H + 14 * MIN }
      ],
      modelsBreakdown: [
        { model: 'gpt-5.3-codex', tokens: 1.24e9, cost: 358.40, share: 84 },
        { model: 'o4-mini-high', tokens: 0.24e9, cost: 54.40, share: 16 }
      ],
      recentFolders: ['D:\\Projects\\abushanab-net\\HarnessManager', 'D:\\Projects\\resal\\sanduq-api']
    },
    {
      id: 'opencode-main',
      name: 'opencode-main',
      isMain: true,
      tool: 'opencode',
      kind: 'routed',
      plan: 'Routed · cheap-fast',
      route: 'cheap-fast (DeepSeek V4 → OpenRouter)',
      email: 'router://127.0.0.1:8787/cheap-fast',
      folder: '~\\.config\\opencode',
      added: 'Nov 1',
      readAgoSec: 35,
      measured: false,
      advice: 'No vendor plan window (—) · $11.20 of $25.00 daily router cap used (resets 00:00).',
      adviceTone: 'neutral',
      openSessions: 3,
      computers: ['developmentpc', 'macbook'],
      providerId: 'router-deepseek',
      firstReplySec: 0.6,
      tokens7d: 318.4e6,
      cost7d: 19.85,
      spark7d: [0.22, 0.35, 0.28, 0.44, 0.51, 0.39, 0.45],
      cap: { limitUsd: 25.0, spentUsd: 11.20, resetMs: (8 * 60 + 18) * MIN },
      windows: [],
      modelsBreakdown: [
        { model: 'deepseek-v4', tokens: 268.0e6, cost: 14.10, share: 84 },
        { model: 'qwen-3-coder', tokens: 50.4e6, cost: 5.75, share: 16 }
      ],
      recentFolders: ['D:\\Projects\\dgx\\spark-serve', '~/Projects/abushanab-net/HarnessManager']
    },
    {
      id: 'agy-main',
      name: 'agy-main',
      isMain: true,
      tool: 'agy',
      kind: 'subscription',
      plan: 'Workspace Pro',
      email: 'samy@example.com',
      folder: '~\\.antigravity',
      added: 'Nov 2',
      readAgoSec: null,
      measured: false,
      needsLogin: true,
      advice: 'Limits not read (—): Google OAuth session expired. Re-authenticate to read windows.',
      adviceTone: 'danger',
      openSessions: 0,
      computers: [],
      providerId: 'google',
      firstReplySec: null,
      tokens7d: 12.4e6,
      cost7d: 9.15,
      spark7d: [0.30, 0.25, 0.12, 0.0, 0.0, 0.0, 0.0],
      windows: [],
      modelsBreakdown: [
        { model: 'gemini-3-pro', tokens: 12.4e6, cost: 9.15, share: 100 }
      ],
      recentFolders: ['D:\\Projects\\experiments\\flutter-canvas']
    }
  ];

  // 4 extra accounts when testing "12 accounts" scale mode (brief.md: "Designs must hold up to 12 accounts comfortably")
  const EXTRA_4_ACCOUNTS = [
    {
      id: 'codex-resal',
      name: 'codex-resal',
      isMain: false,
      tool: 'codex',
      kind: 'subscription',
      plan: 'Team',
      email: 's.abushanab@resal.me',
      folder: '~\\.codex-resal',
      added: 'Oct 30',
      readAgoSec: 65,
      measured: true,
      advice: '82% of week left · resets in 3 d 11 h. Strong Codex fallback when codex-main depletes.',
      adviceTone: 'good',
      openSessions: 0,
      computers: ['developmentpc'],
      providerId: 'openai',
      firstReplySec: 0.9,
      tokens7d: 210.5e6,
      cost7d: 64.20,
      spark7d: [0.15, 0.22, 0.19, 0.31, 0.24, 0.18, 0.25],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 9, left: 91, resetMs: (3 * 60 + 50) * MIN, forecastAtReset: 20, outMs: null },
        { id: 'week', label: 'Week', kind: 'week', used: 18, left: 82, resetMs: (3 * 24 + 11) * H, forecastAtReset: 41, outMs: null }
      ],
      modelsBreakdown: [{ model: 'gpt-5.3-codex', tokens: 210.5e6, cost: 64.20, share: 100 }],
      recentFolders: ['D:\\Projects\\resal\\sanduq-api']
    },
    {
      id: 'claude-audit',
      name: 'claude-audit',
      isMain: false,
      tool: 'claude',
      kind: 'subscription',
      plan: 'Pro',
      email: 'audit@resal.me',
      folder: '~\\.claude-audit',
      added: 'Oct 21',
      readAgoSec: 90,
      measured: true,
      exhaustionAlert: true,
      advice: 'Exhausted on 5-hour window (0% left, resets in 42 m) · 14% week left.',
      adviceTone: 'danger',
      openSessions: 0,
      computers: ['samymsi'],
      providerId: 'anthropic',
      firstReplySec: 1.2,
      tokens7d: 390.0e6,
      cost7d: 142.10,
      spark7d: [0.72, 0.80, 0.85, 0.91, 0.88, 0.95, 1.0],
      windows: [
        { id: '5h', label: '5-hour', kind: '5h', used: 100, left: 0, resetMs: 42 * MIN, forecastAtReset: 100, outMs: 0 },
        { id: 'week', label: 'Week', kind: 'week', used: 86, left: 14, resetMs: (2 * 24 + 6) * H, forecastAtReset: 100, outMs: 19 * H }
      ],
      modelsBreakdown: [{ model: 'claude-sonnet-4-5', tokens: 390.0e6, cost: 142.10, share: 100 }],
      recentFolders: ['D:\\Projects\\resal\\security-audit']
    },
    {
      id: 'pi-openrouter',
      name: 'pi-openrouter',
      isMain: true,
      tool: 'pi',
      kind: 'apikey',
      plan: 'API Key · OpenRouter',
      email: 'sk-or-v1-••••9a4c',
      folder: '~\\.pi',
      added: 'Nov 1',
      readAgoSec: 50,
      measured: false,
      advice: 'API key account (—) · $4.80 of $40.00 monthly cap spent.',
      adviceTone: 'neutral',
      openSessions: 0,
      computers: ['developmentpc'],
      providerId: 'openrouter',
      firstReplySec: 1.3,
      tokens7d: 44.2e6,
      cost7d: 4.80,
      spark7d: [0.08, 0.12, 0.05, 0.18, 0.14, 0.09, 0.11],
      cap: { limitUsd: 40.0, spentUsd: 4.80, resetMs: 5 * DAY },
      windows: [],
      modelsBreakdown: [{ model: 'kimi-k2.5', tokens: 44.2e6, cost: 4.80, share: 100 }],
      recentFolders: ['D:\\Projects\\abushanab-net\\HarnessManager']
    },
    {
      id: 'omp-sandbox',
      name: 'omp-sandbox',
      isMain: false,
      tool: 'omp',
      kind: 'routed',
      plan: 'Routed · fast-coder',
      route: 'fast-coder (Local Router :8787)',
      email: 'router://127.0.0.1:8787/fast-coder',
      folder: '~\\.omp-sandbox',
      added: 'Nov 3',
      readAgoSec: null,
      measured: false,
      advice: 'Prepared profile (—) · install Oh My Pi binary to launch sessions.',
      adviceTone: 'neutral',
      openSessions: 0,
      computers: [],
      providerId: 'router-deepseek',
      firstReplySec: 1.2,
      tokens7d: 0,
      cost7d: 0,
      spark7d: [0, 0, 0, 0, 0, 0, 0],
      windows: [],
      modelsBreakdown: [],
      recentFolders: ['D:\\Projects\\abushanab-net\\HarnessManager']
    }
  ];

  // Live running sessions (12 on DEVELOPMENTPC + 4 on SAMYMSI + 2 on Samys-MacBook-Pro = 18 total: "12 live · 6 elsewhere")
  const LIVE_SESSIONS = [
    { id: 's1', project: 'HarnessManager', branch: 'feat/overview-c', computer: 'developmentpc', accountId: 'claude-main', tool: 'claude', model: 'claude-opus-4-6', sinceMin: 72, tokensSession: 4.82e6, status: 'streaming', lastAction: 'Refactoring timeline lane renderer' },
    { id: 's2', project: 'sanduq-api', branch: 'fix/settlement-race', computer: 'developmentpc', accountId: 'codex-main', tool: 'codex', model: 'gpt-5.3-codex', sinceMin: 34, tokensSession: 2.98e6, status: 'streaming', lastAction: 'Running cargo test --workspace' },
    { id: 's3', project: 'resal-wiki', branch: 'main', computer: 'developmentpc', accountId: 'service-account-resal', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 11, tokensSession: 0.64e6, status: 'streaming', lastAction: 'Syncing architecture diagrams' },
    { id: 's4', project: 'spark-serve', branch: 'vllm-quant', computer: 'developmentpc', accountId: 'opencode-main', tool: 'opencode', model: 'deepseek-v4', sinceMin: 128, tokensSession: 3.12e6, status: 'idle', lastAction: 'Waiting for prompt in terminal #4' },
    { id: 's5', project: 'checkout-service', branch: 'stripe-v2', computer: 'developmentpc', accountId: 'sabushanab-resal', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 47, tokensSession: 1.95e6, status: 'streaming', lastAction: 'Verifying webhook idempotency' },
    { id: 's6', project: 'HarnessManager', branch: 'tauri-tray-sync', computer: 'developmentpc', accountId: 'claude-main', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 19, tokensSession: 1.18e6, status: 'idle', lastAction: 'Ready for review' },
    { id: 's7', project: 'core-ledger', branch: 'audit-log', computer: 'developmentpc', accountId: 'claude-main', tool: 'claude', model: 'claude-opus-4-6', sinceMin: 95, tokensSession: 6.40e6, status: 'streaming', lastAction: 'Analyzing SQL partition migration' },
    { id: 's8', project: 'sanduq-api', branch: 'openapi-spec', computer: 'developmentpc', accountId: 'codex-main', tool: 'codex', model: 'gpt-5.3-codex', sinceMin: 23, tokensSession: 1.42e6, status: 'idle', lastAction: 'Generated 14 endpoint schemas' },
    { id: 's9', project: 'HarnessManager', branch: 'e2e-playwright', computer: 'developmentpc', accountId: 'codex-main', tool: 'codex', model: 'o4-mini-high', sinceMin: 15, tokensSession: 0.89e6, status: 'streaming', lastAction: 'Executing visual regression suite' },
    { id: 's10', project: 'merchant-portal', branch: 'rtl-polish', computer: 'developmentpc', accountId: 'claude-main', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 61, tokensSession: 2.30e6, status: 'idle', lastAction: 'Completed RTL table alignment' },
    { id: 's11', project: 'spark-serve', branch: 'cuda-graphs', computer: 'developmentpc', accountId: 'opencode-main', tool: 'opencode', model: 'deepseek-v4', sinceMin: 84, tokensSession: 2.15e6, status: 'streaming', lastAction: 'Profiling prefill latency' },
    { id: 's12', project: 'resal-wiki', branch: 'runbooks', computer: 'developmentpc', accountId: 'claude-main', tool: 'claude', model: 'claude-haiku-4-5', sinceMin: 8, tokensSession: 0.31e6, status: 'idle', lastAction: 'Indexed 42 markdown pages' },
    // 4 on SAMYMSI
    { id: 's13', project: 'edge-gateway', branch: 'main', computer: 'samymsi', accountId: 'samykabu-teksa', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 52, tokensSession: 3.04e6, status: 'streaming', lastAction: 'Compiling Rust proxy filters' },
    { id: 's14', project: 'telemetry-ui', branch: 'webgl-grid', computer: 'samymsi', accountId: 'samykabu-teksa', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 29, tokensSession: 1.76e6, status: 'idle', lastAction: 'Updated shader uniforms' },
    { id: 's15', project: 'model-adoption', branch: 'qwen3-eval', computer: 'samymsi', accountId: 'codex-main', tool: 'codex', model: 'gpt-5.3-codex', sinceMin: 41, tokensSession: 2.64e6, status: 'streaming', lastAction: 'Benchmarking throughput on 4090' },
    { id: 's16', project: 'checkout-service', branch: 'load-test', computer: 'samymsi', accountId: 'sabushanab-resal', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 17, tokensSession: 0.92e6, status: 'streaming', lastAction: 'Running k6 stress profile' },
    // 2 on Samys-MacBook-Pro
    { id: 's17', project: 'synth-webgl', branch: 'main', computer: 'macbook', accountId: 'samykabu-personal', tool: 'claude', model: 'claude-sonnet-4-5', sinceMin: 64, tokensSession: 1.88e6, status: 'streaming', lastAction: 'Tuning audio-reactive particles' },
    { id: 's18', project: 'blog', branch: 'draft/ai-harnesses', computer: 'macbook', accountId: 'opencode-main', tool: 'opencode', model: 'deepseek-v4', sinceMin: 22, tokensSession: 0.54e6, status: 'idle', lastAction: 'Proofreading draft notes' }
  ];

  // Pinned projects in sidebar (under Projects fold)
  const PINNED_PROJECTS = [
    { name: 'HarnessManager', path: 'D:\\Projects\\abushanab-net\\HarnessManager', live: 3 },
    { name: 'sanduq-api', path: 'D:\\Projects\\resal\\sanduq-api', live: 2 },
    { name: 'checkout-service', path: 'D:\\Projects\\resal\\checkout-service', live: 2 },
    { name: 'spark-serve', path: 'D:\\Projects\\dgx\\spark-serve', live: 2 },
    { name: 'resal-wiki', path: 'D:\\Projects\\resal\\resal-wiki', live: 2 }
  ];

  // Readouts by range (Today / 7 d / 30 d / All) with dot-grid arrays (24 cells = 3 rows × 8 cols)
  const READOUTS = {
    today: {
      label: 'Today',
      tokens: { val: '2.41B', delta: '+18%', deltaTone: 'up', sub: 'vs 2.04B yesterday by 15:42', grid: [1,1,2,1,2,2,3,3, 1,2,2,3,3,3,4,4, 2,3,3,4,4,4,4,4] },
      cost:   { val: '$571.37', delta: '-$256', deltaTone: 'down', sub: 'at API list prices (≈) · $11.20 billed', grid: [2,2,1,2,2,3,2,3, 2,3,2,3,3,2,3,3, 3,2,3,3,4,3,4,3] },
      cached: { val: '98%', delta: '+0 pt', deltaTone: 'flat', sub: '2.36B cache read of 2.41B total', grid: [4,4,4,4,4,4,4,4, 4,4,3,4,4,4,4,4, 4,4,4,4,4,4,4,4] }
    },
    '7d': {
      label: '7 d',
      tokens: { val: '9.97B', delta: '+24%', deltaTone: 'up', sub: 'vs 8.04B prior 7 days', grid: [2,2,3,2,3,3,4,4, 2,3,3,2,3,4,4,4, 3,3,4,2,3,4,4,4] },
      cost:   { val: '$3,492.43', delta: '+$412', deltaTone: 'up', sub: 'at API list prices (≈) · $78.40 billed', grid: [2,3,3,2,3,3,4,4, 2,2,4,2,3,3,4,4, 3,3,4,2,3,4,4,4] },
      cached: { val: '96%', delta: '+2 pt', deltaTone: 'up', sub: '9.57B cache hits across 4 tools', grid: [3,4,4,4,4,4,4,4, 4,4,4,3,4,4,4,4, 4,4,4,4,4,4,4,4] }
    },
    '30d': {
      label: '30 d',
      tokens: { val: '38.4B', delta: '+41%', deltaTone: 'up', sub: 'vs 27.2B prior 30 days', grid: [1,2,2,2,3,3,3,4, 2,2,3,3,3,4,4,4, 2,3,3,3,4,4,4,4] },
      cost:   { val: '$13,180', delta: '+$2,840', deltaTone: 'up', sub: 'at API list prices (≈) · $312 billed', grid: [1,2,2,3,3,3,4,4, 2,2,3,3,3,4,4,4, 2,3,3,4,4,4,4,4] },
      cached: { val: '95%', delta: '+3 pt', deltaTone: 'up', sub: 'steady prompt cache reuse', grid: [3,3,4,4,4,4,4,4, 3,4,4,4,4,4,4,4, 4,4,4,4,4,4,4,4] }
    },
    all: {
      label: 'All',
      tokens: { val: '142.8B', delta: '53 wks', deltaTone: 'flat', sub: 'since Nov 2025 across 3 computers', grid: [1,1,1,2,2,3,3,4, 1,1,2,2,3,3,4,4, 1,2,2,3,3,4,4,4] },
      cost:   { val: '$48,920', delta: 'list ≈', deltaTone: 'flat', sub: 'saved ~$46.8k via subscriptions', grid: [1,1,2,2,2,3,3,4, 1,2,2,2,3,3,4,4, 1,2,2,3,3,4,4,4] },
      cached: { val: '94%', delta: 'lifetime', deltaTone: 'flat', sub: '134.2B cached tokens read', grid: [3,3,3,4,4,4,4,4, 3,3,4,4,4,4,4,4, 3,4,4,4,4,4,4,4] }
    }
  };

  // Needs attention items (matching home-dark.png + tool update / login notice)
  const NEEDS_ATTENTION = [
    {
      id: 'att-1',
      severity: 'warn',
      title: 'codex-main week',
      when: '≈ Sun 00:56',
      reason: 'Runs out 2 d 14 h before its Tue 15:46 reset at today’s pace.',
      accountId: 'codex-main',
      actionLabel: 'Inspect lane'
    },
    {
      id: 'att-2',
      severity: 'warn',
      title: 'claude-main week · fable',
      when: '≈ Tue 08:04',
      reason: 'Runs out 3 d 4 h before its reset at today’s pace.',
      accountId: 'claude-main',
      actionLabel: 'Inspect lane'
    },
    {
      id: 'att-3',
      severity: 'warn',
      title: 'claude-main week',
      when: '≈ Thu 03:38',
      reason: 'Runs out 1 d 9 h before its reset at today’s pace.',
      accountId: 'claude-main',
      actionLabel: 'Inspect lane'
    },
    {
      id: 'att-4',
      severity: 'danger',
      title: 'agy-main login expired',
      when: 'Not read (—)',
      reason: 'Google OAuth token expired; limits cannot be read until re-authenticated.',
      accountId: 'agy-main',
      actionLabel: 'Sign in'
    },
    {
      id: 'att-5',
      severity: 'info',
      title: 'Codex 0.168.0 update ready',
      when: 'Apps · 1 update',
      reason: 'Installed 0.167.0 on DEVELOPMENTPC; 0.168.0 adds faster diff apply.',
      accountId: null,
      actionLabel: 'Update tool'
    }
  ];

  // Daily stacked bar chart for Accounts page (7d / 14d / 30d)
  const DAILY_STACK_SERIES = (function () {
    const rng = (seed) => () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
    const r = rng(108);
    const out = [];
    for (let i = 29; i >= 0; i--) {
      const d = new Date(NOW - i * DAY);
      const label = i === 0 ? 'today' : d.toLocaleDateString('en-US', { month: 'short', day: 'numeric' });
      const weekend = d.getDay() === 0 || d.getDay() === 6 ? 0.55 : 1.0;
      const claudeMain = Math.round((700 + r() * 650) * weekend); // M tokens
      const codexMain = Math.round((140 + r() * 160) * weekend);
      const opencodeMain = Math.round((30 + r() * 45) * weekend);
      const sabushanab = Math.round((15 + r() * 30) * weekend);
      const teksa = Math.round((35 + r() * 50) * weekend);
      const personal = Math.round((5 + r() * 18) * weekend);
      const serviceResal = i < 3 ? Math.round(1 + r() * 3) : 0;
      out.push({
        dayOffset: i,
        label,
        totalM: claudeMain + codexMain + opencodeMain + sabushanab + teksa + personal + serviceResal,
        parts: [
          { id: 'claude-main', col: '#3FD6CF', valM: claudeMain },
          { id: 'codex-main', col: '#7A9DFF', valM: codexMain },
          { id: 'samykabu-teksa', col: '#9E8CFC', valM: teksa },
          { id: 'sabushanab-resal', col: '#57CAC9', valM: sabushanab },
          { id: 'opencode-main', col: '#E5A93C', valM: opencodeMain },
          { id: 'samykabu-personal', col: '#6FA8F5', valM: personal },
          { id: 'service-account-resal', col: '#2CE59B', valM: serviceResal }
        ]
      });
    }
    return out;
  })();

  // 53 weeks × 7 days radial heatmap data for the Year Clock
  const YEAR_CLOCK_DAYS = (function () {
    const rng = (seed) => () => { seed = (seed * 16807) % 2147483647; return (seed - 1) / 2147483646; };
    const r = rng(77);
    const cells = [];
    for (let w = 0; w < 53; w++) {
      for (let d = 0; d < 7; d++) {
        const idx = w * 7 + d;
        const activeSeason = w >= 34 && w <= 45; // Sep–Nov peak matching screenshot
        let intensity = activeSeason ? (0.25 + r() * 0.75) : (r() < 0.28 ? r() * 0.32 : 0.03);
        if (d === 5 || d === 6) intensity *= 0.55;
        // Peak on Sep 30
        const isPeak = w === 40 && d === 2;
        if (isPeak) intensity = 1.0;
        const tokensB = +(intensity * 4.79).toFixed(2);
        cells.push({
          week: w,
          day: d,
          idx,
          intensity,
          tokensB,
          byComputer: {
            all: intensity,
            developmentpc: +(intensity * (0.62 + r() * 0.15)).toFixed(2),
            samymsi: +(intensity * (0.25 + r() * 0.12)).toFixed(2),
            macbook: +(intensity * (0.12 + r() * 0.08)).toFixed(2)
          }
        });
      }
    }
    return cells;
  })();

  // Latest replies (10,756 today · speed and first reply are approximate)
  // NOTE: Rule 2 forbids tokens-per-second anywhere on the Overview!
  // On the Overview table, "Speed" is shown as qualitative cadence ("Fast · steady", "Normal", etc.)
  // while the full Request Log modal includes the full numeric breakdown.
  const LATEST_REPLIES = [
    { time: '15:41:52', ago: '8s ago', account: 'claude-main', tool: 'claude', model: 'claude-opus-4-6', computer: 'DEVELOPMENTPC', computerId: 'developmentpc', tokens: '148.2k', inTok: '4,120', outTok: '1,840', cacheTok: '142,240', paceLabel: 'Steady stream', tpsModalOnly: '94 tok/s', firstReply: '≈ 1.1 s', cost: '$0.14' },
    { time: '15:41:34', ago: '26s ago', account: 'codex-main', tool: 'codex', model: 'gpt-5.3-codex', computer: 'DEVELOPMENTPC', computerId: 'developmentpc', tokens: '84.6k', inTok: '2,910', outTok: '1,190', cacheTok: '80,500', paceLabel: 'Fast burst', tpsModalOnly: '132 tok/s', firstReply: '≈ 0.8 s', cost: '$0.07' },
    { time: '15:41:09', ago: '51s ago', account: 'service-account-resal', tool: 'claude', model: 'claude-sonnet-4-5', computer: 'DEVELOPMENTPC', computerId: 'developmentpc', tokens: '62.4k', inTok: '1,840', outTok: '920', cacheTok: '59,640', paceLabel: 'Fast burst', tpsModalOnly: '118 tok/s', firstReply: '≈ 0.9 s', cost: '$0.04' },
    { time: '15:40:41', ago: '1m ago', account: 'samykabu-teksa', tool: 'claude', model: 'claude-sonnet-4-5', computer: 'SAMYMSI', computerId: 'samymsi', tokens: '119.0k', inTok: '3,400', outTok: '1,480', cacheTok: '114,120', paceLabel: 'Steady stream', tpsModalOnly: '104 tok/s', firstReply: '≈ 1.1 s', cost: '$0.08' },
    { time: '15:40:12', ago: '1m ago', account: 'opencode-main', tool: 'opencode', model: 'deepseek-v4', computer: 'DEVELOPMENTPC', computerId: 'developmentpc', tokens: '53.8k', inTok: '2,100', outTok: '1,310', cacheTok: '50,390', paceLabel: 'Low latency', tpsModalOnly: '146 tok/s', firstReply: '≈ 0.6 s', cost: '$0.01' },
    { time: '15:39:48', ago: '2m ago', account: 'sabushanab-resal', tool: 'claude', model: 'claude-sonnet-4-5', computer: 'SAMYMSI', computerId: 'samymsi', tokens: '91.2k', inTok: '2,650', outTok: '1,120', cacheTok: '87,430', paceLabel: 'Steady stream', tpsModalOnly: '109 tok/s', firstReply: '≈ 1.0 s', cost: '$0.06' },
    { time: '15:39:05', ago: '3m ago', account: 'samykabu-personal', tool: 'claude', model: 'claude-sonnet-4-5', computer: 'Samys-MacBook-Pro', computerId: 'macbook', tokens: '44.7k', inTok: '1,420', outTok: '680', cacheTok: '42,600', paceLabel: 'Fast burst', tpsModalOnly: '115 tok/s', firstReply: '≈ 1.2 s', cost: '$0.03' },
    { time: '15:38:29', ago: '3m ago', account: 'codex-main', tool: 'codex', model: 'o4-mini-high', computer: 'SAMYMSI', computerId: 'samymsi', tokens: '76.1k', inTok: '3,110', outTok: '1,540', cacheTok: '71,450', paceLabel: 'Fast burst', tpsModalOnly: '138 tok/s', firstReply: '≈ 0.9 s', cost: '$0.05' }
  ];

  // Available local router routes for Add Account -> Routed
  const ROUTER_ROUTES = [
    { id: 'cheap-fast', name: 'cheap-fast', target: 'DeepSeek V4 → OpenRouter fallback', latency: '0.6s first reply', costNote: '$0.14 / 1M tok' },
    { id: 'frontier-code', name: 'frontier-code', target: 'Claude Sonnet 4.5 → GPT-5.3-Codex fallback', latency: '1.0s first reply', costNote: '$3.00 / 1M tok' },
    { id: 'local-dgx', name: 'local-dgx', target: 'DGX Spark vLLM (Qwen-3-Coder-72B)', latency: '0.4s first reply', costNote: '$0.00 (LAN)' }
  ];

  // Formatting utilities
  function fmtDuration(ms) {
    if (ms == null || isNaN(ms)) return '—';
    const totalMin = Math.max(1, Math.round(ms / MIN));
    if (totalMin < 60) return `${totalMin} m`;
    const totalHours = Math.floor(totalMin / 60);
    const remMin = totalMin % 60;
    if (totalHours < 24) {
      return remMin > 0 ? `${totalHours} h ${String(remMin).padStart(2, '0')} m` : `${totalHours} h`;
    }
    const days = Math.floor(totalHours / 24);
    const remHours = totalHours % 24;
    return remHours > 0 ? `${days} d ${remHours} h` : `${days} d`;
  }

  function fmtAbsoluteReset(msFromNow) {
    if (msFromNow == null) return '—';
    const dt = new Date(NOW + msFromNow);
    const wd = dt.toLocaleDateString('en-US', { weekday: 'short' });
    const hh = String(dt.getHours()).padStart(2, '0');
    const mm = String(dt.getMinutes()).padStart(2, '0');
    if (msFromNow < 18 * H) return `Today ${hh}:${mm}`;
    return `${wd} ${hh}:${mm}`;
  }

  function fmtTokens(n) {
    if (n == null) return '—';
    if (n === 0) return '0';
    if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
    if (n >= 1e6) return (n / 1e6).toFixed(n >= 100e6 ? 1 : 2) + 'M';
    if (n >= 1e3) return (n / 1e3).toFixed(1) + 'k';
    return String(n);
  }

  function fmtMoney(n) {
    if (n == null) return '—';
    return '$' + n.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  }

  // Dynamically derive all summary metrics, recommendations, and shell counts from data
  function deriveMetrics(accounts, preset) {
    const isFresh = preset === 'empty' || preset === 'firstrun';
    const activeSessions = isFresh ? [] : LIVE_SESSIONS.filter((s) => accounts.some((a) => a.id === s.accountId));
    const liveHere = activeSessions.filter((s) => s.computer === 'developmentpc').length;
    const liveElsewhere = activeSessions.length - liveHere;
    const costTodayVal = isFresh ? '$0.00' : READOUTS.today.cost.val;
    const projectsCount = isFresh ? 0 : 23;
    const computersList = isFresh ? [COMPUTERS[0]] : COMPUTERS;
    const readAges = accounts.filter((a) => a.readAgoSec != null).map((a) => a.readAgoSec);
    const freshnessText = (isFresh || readAges.length === 0)
      ? 'Limits not read'
      : (() => {
          const minRead = Math.min(...readAges);
          return `Limits read ${minRead <= 5 ? 'just now' : Math.max(1, Math.round(minRead / 60)) + ' m ago'} · usage just now`;
        })();

    // Best room account (measured account sorted by week.left then 5h.left)
    const measuredAccts = accounts.filter((a) => a.measured && a.windows && a.windows.length);
    const sortedByRoom = measuredAccts.slice().sort((a, b) => {
      const aw = (a.windows.find((w) => w.kind === 'week') || a.windows[0]).left;
      const bw = (b.windows.find((w) => w.kind === 'week') || b.windows[0]).left;
      if (bw !== aw) return bw - aw;
      const a5 = (a.windows.find((w) => w.kind === '5h') || a.windows[0]).left;
      const b5 = (b.windows.find((w) => w.kind === '5h') || b.windows[0]).left;
      return b5 - a5;
    });
    const bestRoomAcct = sortedByRoom[0] || accounts[0] || null;

    // Expiring soonest account with harvestable room (5h window with >= 50% left sorted by earliest resetMs)
    const with5h = measuredAccts.filter((a) => a.windows.some((w) => w.kind === '5h' && w.left >= 50));
    with5h.sort((a, b) => {
      const a5 = a.windows.find((w) => w.kind === '5h').resetMs;
      const b5 = b.windows.find((w) => w.kind === '5h').resetMs;
      return a5 - b5;
    });
    const expiringAcct = with5h[0] || bestRoomAcct;

    // Exhausted or earliest run-out forecast account
    const atRisk = [];
    measuredAccts.forEach((a) => {
      a.windows.forEach((w) => {
        if (w.left === 0 || (w.outMs != null && w.outMs < w.resetMs)) {
          atRisk.push({ acct: a, win: w, outMs: w.left === 0 ? 0 : w.outMs, gapMs: w.resetMs - (w.outMs || 0) });
        }
      });
    });
    atRisk.sort((x, y) => x.outMs - y.outMs);
    const primaryRisk = atRisk[0] || null;

    // Dynamically derived Needs Attention list from atRisk + needsLogin + tool updates
    const attentionItems = [];
    if (!isFresh) {
      atRisk.forEach((r, idx) => {
        const isExh = r.win.left === 0;
        attentionItems.push({
          id: `att-risk-${idx}`,
          severity: isExh ? 'danger' : 'warn',
          title: `${r.acct.name} ${r.win.label.toLowerCase()}`,
          when: isExh ? `0% left · ↻ ${fmtDuration(r.win.resetMs)}` : `≈ ${fmtAbsoluteReset(r.outMs)}`,
          reason: isExh
            ? `Exhausted on ${r.win.label.toLowerCase()} window (resets in ${fmtDuration(r.win.resetMs)}).`
            : `Runs out ${fmtDuration(r.gapMs)} before its ${fmtAbsoluteReset(r.win.resetMs)} reset at today’s pace.`,
          accountId: r.acct.id
        });
      });
      accounts.filter((a) => a.needsLogin).forEach((a) => {
        attentionItems.push({
          id: `att-login-${a.id}`,
          severity: 'danger',
          title: `${a.name} login expired`,
          when: 'Not read (—)',
          reason: 'OAuth token expired; limits cannot be read until re-authenticated.',
          accountId: a.id
        });
      });
      TOOLS.filter((t) => t.updateAvailable).forEach((t) => {
        attentionItems.push({
          id: `att-upd-${t.id}`,
          severity: 'info',
          title: `${t.name} ${t.updateAvailable} update ready`,
          when: 'Apps · 1 update',
          reason: `Installed ${t.ver} on DEVELOPMENTPC; ${t.updateAvailable} available.`,
          accountId: null
        });
      });
    }

    const updatesCount = TOOLS.filter((t) => t.updateAvailable).length;

    // Model aggregates across active accounts
    const modelTotals = {};
    accounts.forEach((a) => {
      (a.modelsBreakdown || []).forEach((m) => {
        if (!modelTotals[m.model]) modelTotals[m.model] = { model: m.model, tokens: 0, cost: 0 };
        modelTotals[m.model].tokens += m.tokens;
        modelTotals[m.model].cost += m.cost;
      });
    });
    const modelsList = Object.values(modelTotals).sort((a, b) => b.tokens - a.tokens);

    // Peak day and 53-week total from YEAR_CLOCK_DAYS and COMPUTERS
    let peakCell = YEAR_CLOCK_DAYS[0];
    YEAR_CLOCK_DAYS.forEach((c) => {
      if (c.tokensB > peakCell.tokensB) peakCell = c;
    });
    const sum53w = COMPUTERS.reduce((s, c) => s + c.tokens53w, 0);

    return {
      isFresh,
      activeSessions,
      liveHere,
      liveElsewhere,
      costTodayVal,
      projectsCount,
      computersList,
      freshnessText,
      bestRoomAcct,
      expiringAcct,
      atRisk,
      primaryRisk,
      attentionItems,
      updatesCount,
      repliesTodayCount: isFresh ? 0 : 10756,
      modelsList,
      peakDayLabel: 'Sep 30',
      peakDayVal: peakCell.tokensB.toFixed(2) + 'B',
      totalYearVal: (sum53w / 1e9).toFixed(1) + 'B'
    };
  }

  window.HM_DATA = {
    NOW,
    MIN,
    H,
    DAY,
    LOGOS,
    COMPUTERS,
    TOOLS,
    BASE_ACCOUNTS,
    EXTRA_4_ACCOUNTS,
    LIVE_SESSIONS,
    PINNED_PROJECTS,
    READOUTS,
    NEEDS_ATTENTION,
    DAILY_STACK_SERIES,
    YEAR_CLOCK_DAYS,
    LATEST_REPLIES,
    ROUTER_ROUTES,
    fmtDuration,
    fmtAbsoluteReset,
    fmtTokens,
    fmtMoney,
    deriveMetrics
  };
})();

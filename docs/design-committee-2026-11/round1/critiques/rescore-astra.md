# Astra (Codex): re-score, round 1

Codex’s five-hour exhaustion is valid. The capability matrix used here is API key: Claude Code, Codex, Pi; Routed: Claude Code, Pi; all other tools subscription-only.

## Design A (Claude) — score 7/10, previously 6

Judged from revised code only; no screenshots were used.

**Must-change items now fixed**

- `winHTML()` shows reset and forecast together. The ladder now separates weekly ring, tightest-window capacity and reset.
- `tick()` calls `patchStrip()` alongside the board and ladder. The prominent answer no longer stays frozen while supporting countdowns change.
- `outFirst()` explicitly prioritises exhausted windows. The twelve-account fixture supplies a real zero-remaining case.
- Overview replies and its inline request log no longer show throughput.
- Add now appends the entered account with unread limits, rejects duplicate names and preserves the chosen data preset. Its capability matrix matches the owner’s correction.
- Add, detail, Follow and folder surfaces have focus wrapping and opener restoration. Four-card lane limits and an independently scrolling ladder address uncontrolled vertical growth.

**Still open**

- **Freshness at 1280 is not fixed.** The shortened text exists, but the later `.app[data-w="1280"] .fresh{display:none}` still hides its parent (`styles.css:490`).
- **The shared snapshot fix stops at the drawer.** `patchDrawer()` updates session telemetry, but never refreshes its mounted limit-window countdowns. Accounts, detail and tray values also remain outside the periodic update path.
- First-run Accounts rows are unread clones, while `detail()` resolves their IDs through `acct()` back to populated fixtures. Opening an unread main profile consequently reveals measured limits and existing sessions. First-run model totals also remain populated.
- Export, rename, hide/remove and settings Save still announce success without performing the corresponding local state change. The account-detail thirty-day chart still uses a sine function rather than usage data.
- The wizard’s extra steps and wide-screen lane composition remain deferred. Screenshot-free code review cannot confirm the claimed fold, clipping or complete contrast results.

**Defects exposed by the fixes**

- Newly created Claude/Codex API-key accounts—and Claude routed accounts—offer a misleading limit-reading path. `noQuota()` checks the tool, not the account kind; Read can manufacture subscription windows for them.
- Adding an account therefore works structurally, but the subsequent unknown/no-plan distinction is incomplete.

**Designer disagreements**

- **Accept retaining the write pulse:** it communicates recent activity at a glance, while the timestamp supplies precision; Still and reduced-motion remain necessary.
- **Accept excluding prototype controls from product Tab-order criticism.** That does not establish that the remaining shell order is optimal.
- **Accept retaining tool/computer filters:** the inventory explicitly requires them.
- **Accept the readouts rebuttal:** `overview()` creates them unconditionally; filtering does not create the section.
- **Accept removing the sidebar:** the brief explicitly permits it.
- **Accept deferring application integration, including the drawer/render-model mapping.** That deferral does not excuse the prototype’s stale drawer values.
- **Do not accept official icons alone as sufficient novice labels at 1280.** The brief requires official icons, not removal of their names.

## Design B (Codex) — score 8/10, previously 7

Judged from revised code and all 37 refreshed screenshots, covering the three widths, both themes and the supplied secondary states.

**Must-change items now fixed**

- The exception is the headline. Across the Overview screenshots, “codex-main is out of its 5-hour window” delivers the answer immediately.
- Reset/state text is enlarged, and forecasts explicitly name **Weekly**. Five-hour exhaustion and a later weekly forecast now coexist intelligibly.
- Each of the six running projects has its own session target, account and named computer. The wrong-project Follow problem is fixed.
- Accounts starts substantially earlier. At 1280, both themes show four complete account rows before the next begins.
- `spark()` reads the account’s actual daily fixture series; decorative KPI grids are removed. The expanded stacked chart has account colours, a legend and a stated zero-based scale.
- Data tick changes a quota and invokes `render()`. The renderer restores keyed focus, disclosures and content scroll; modal forms remain mounted. Animation delays use an elapsed clock. This is materially stronger code evidence than the previous freshness-only tick.
- Add uses the corrected capability matrix. Newly created accounts remain unread; capped routes and API-key accounts have separate treatments. The default subscription form fits the supplied 720px-high captures.

**Still open**

- **The detected-profile onboarding transition imports populated demo state.** `detectedcontinue` selects existing fixture main profiles without clearing their readings, usage or sessions. A direct state check produced four running sessions and measured Claude/Codex windows immediately after detection, while parts of the shell still describe first-run state.
- **Recommendations lack an unavailable-now guard.** `bestAccount()` can select an exhausted subscription when every measured account is exhausted. With all fixture windows set to 100% used, it recommended `codex-main`.
- Overview labels `agy-main` merely “Not read,” concealing its expired login until disclosure/detail. `opencode-main`, whose tool exposes no quota readings, receives the same state label.
- Focus restoration is stronger than data propagation: open detail/session/tray dialogs remain mounted with their original readings. A quota-changing redraw updates the ledger without updating an already-open detail’s limits.
- The attention section still hardcodes quota values and reset text. Recommendations and secondary warnings can diverge under the new quota-changing tick.

**Defects introduced or exposed by the fixes**

- The new detected-profile review flow reintroduces canned activity during onboarding.
- The genuine quota-changing tick exposes stale modal readings and hardcoded attention copy that the old label-only tick could not reveal.

**Designer disagreements**

- **Accept disclosure for rank-three attention, activity and replies**, and for Setup destinations.
- **Accept retaining account details, usage, launch arguments and permission controls:** these preserve inventory capabilities.
- **Accept a separate subscription sign-in step:** it represents required tool behaviour.
- **Accept keeping the limits-first ledger:** the revised project rows address Follow without abandoning B’s assigned angle.
- **Accept deferring native polling, tray and platform acceptance.** The prototype remains a strong base needing another round; its verification report does not establish those environments.

## Design C (OpenCode) — score 6/10, previously 4

Judged from revised code only; no screenshots were used.

**Must-change items now fixed**

- Capacity is separated from time. Reset gates and axis ticks now share a linear seven-day calculation.
- Forecast dashes require a forecast before reset; healthy accounts no longer receive an invented pace trajectory.
- Risk selection and attention items are derived from account windows. The twelve-account exhausted fixture now actually has 0% left.
- The default running sample includes a named project from each computer, correcting the exclusively local initial sample.
- Add-field typing patches previews without replacing the input. Account creation retains the current preset and starts with `measured:false` and empty windows.
- Follow opens a session surface; removal changes the active list; CSV constructs a real download. Account rows have Enter/Space activation.
- The daily chart starts folded, the year clock uses discrete cells, and modal focus handling now exists.

**Still open**

- **The capability-matrix fix is incorrect.** `data.js` still enables Routed for Codex, OpenCode and Oh My Pi; fixtures also contain unsupported OpenCode/Oh My Pi routed accounts. Subscription descriptions still promise readable windows for tools that expose none.
- **Timeline honesty is improved, not complete.** `renderLinearRunwayTrack()` still expands starvation gaps with `Math.max(4, spanPct)` and imposes a minimum five-hour fill width. Those marks can disagree with the otherwise linear gates.
- **Unknown availability still becomes a recommendation.** `deriveMetrics()` falls back to `accounts[0]` without measured capacity. Rendering a single newly added unread account produced a “ready” hero and “most room” recommendation.
- **“Every summary is derived” is false.** That same empty-preset creation path rendered existing cost readouts and fixture replies beside a zero reply count. First-run main-profile clones also retain old usage, advice and session counts.
- Running visibility remains partial: All shows one session per computer; individual computer filters stop at three. The top live-status action opens an out-of-round placeholder.
- Redraw preservation is incomplete. Recent-folder selection is not stored, and focus selectors omit distinguishing attributes such as `data-expiring`, allowing restoration to another control.
- Add still lacks duplicate-name and meaningful credential/sign-in validation. Account settings omit required editable fields; the account menu still offers removal wording for main profiles despite their hide-only rule.

**Defects introduced or exposed by the fixes**

- The unread-account creation fix now exposes contradictory “ready,” historical-usage and fixture-reply states immediately afterward.
- The linear seven-day replacement compresses every five-hour reset into the first 3% of the track. Its arithmetic is substantially better, but it contributes little visual discrimination for the owner’s immediate reset decision.

**Designer disagreements**

- No explicit disagreement is offered.
- **Accept deferring consolidation of inline styles into the application stylesheet.**
- **Reject deferring responsive correctness itself:** viewport breakpoints and a frame-width switch must already agree in the prototype. Nor can integration deferral cover incorrect capabilities, unknown-state recommendations or remaining positional distortion.

## Across the three

- **Must have:** one current, readable answer connecting unavailable accounts, eligible next accounts, named limit windows/resets and directly followable projects on named computers.
- **Must not:** turn unread data, unsupported capabilities, distorted geometry or a simulated success into operational certainty.
- **Parts worth combining:** A’s computer/project board and keyed session updates.
- **Parts worth combining:** B’s aligned capacity ledger, explicit forecast-window labels and compact Accounts introduction.
- **Parts worth combining:** B’s tool-first Add flow and separate cap/pay-per-use treatments.
- **Parts worth combining:** C’s distinction between maximum room and capacity worth using before reset, after enforcing eligibility and honest unknown states.
- **Ranking: B > A > C.** B makes the clearest verified visual improvement; A retains the strongest running/where structure; C has repaired major foundations but still contradicts its own instruments and account states.

Checks not performed: browser interaction reruns, native WebView2/WKWebView, macOS or production polling; A and C received code-only assessment.  
Remaining risk: all three still have paths where recommendations or secondary readings disagree with current account state.
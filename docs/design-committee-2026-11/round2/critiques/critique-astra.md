# Astra (Codex): critique, round 2

## The shell and the base (Overview, Accounts, Account detail, Add account, tray) — score 6/10

Judged from the source and supplied screenshots, including the original app captures. No browser was opened. Additional checks executed the renderers and state handlers without a browser; these establish code behavior, not rendered interaction or native acceptance.

**Still open from round 1**

- Eligibility and unknown states remain incomplete. `deriveMetrics()` excludes expired accounts from “Most room,” but its expiring-soon candidate does not apply the same guard (`data.js:876–885`). A code-only check selected an expired `sabushanab-resal`.
- The absence of an eligible recommendation still produces an incorrect operational answer. With every measured window exhausted, the renderer says “Limits have not been read yet” and removes the dispatch deck and horizon (`views.js:178`). The check retained **18 active sessions** in the data while rendering neither Running now nor Reset Horizon.
- Account detail still has a separate history from the rest of the app. The selected account reports 34 seven-day sessions; Designer A’s session dataset contains 13 for it in that range. Its `recent-1` and `recent-2` links have no corresponding Session records.

**What works**

- The phase-one repairs materially improve C. Capacity and time are separate; the five-hour track explicitly names its zoom; weekly gates use consistent fractions; starvation intervals no longer receive artificial minimum widths.
- The supplied 1280 captures show a complete dispatch deck, named work on all three computers, readable first lanes and an unclipped twelve-account tray. This is a substantial improvement over my previous code-only assessment.
- Add account implements the corrected capability matrix. Unsupported choices are disabled; new accounts retain unread limits; main-profile removal becomes hide-only.
- `patchNode()` reconciles existing elements. This is a credible vanilla-JS foundation, materially stronger than the original full-root replacement.
- Accounts begins with useful rows after folded charts. Account detail retains used-share bars and explicitly labels their forecast marks.

**What fails**

- **All widths/themes, code-confirmed:** exhausted, unread and usage-only installations converge on onboarding. Those are different operating conditions, and existing work disappears precisely when the owner needs it most.
- **All widths/themes:** account detail’s Sessions tab shows active sessions plus two canned recent entries rather than the required session rows and history. Most other accounts show missing detail statistics despite populated usage elsewhere.
- Refresh announces a preview but also changes freshness to “just now” without obtaining new readings (`modals.js:1018–1028`). The timestamp claim remains false after the explanatory toast is dismissed.
- Add account advertises API-key spend caps although the inventory assigns caps to routed accounts. Its router text and choices still use `:8787` and `D.ROUTER_ROUTES`; Designer B’s routing page uses `:15731` and another route collection.
- The base folder dialog and A’s New session dialog differ in account eligibility, permission flags and folder selection. Accounts also retains the explanation sentence the owner wanted removed.
- The ring still expands a nonzero percentage to at least 2% (`views.js:28`). The time repair did not remove every dishonest minimum.

**Must change**

Keep C’s composition. Redraw its unavailable-capacity state so running work, exhausted windows and resets remain visible without a recommendation. Use one eligibility predicate and one account/session/route/settings snapshot throughout. Make freshness reflect the last actual reading. Complete account history navigation and remove the unsupported spend-cap promise.

Within the base: **Overview 6/10; Accounts 7/10; Account detail 5/10; Add account 7/10; tray 8/10.**

## Sessions — 6/10

**What works**

- The finding answers a real question. Rows connect title, account, tool, computer, tokens and action without an explanation paragraph.
- Grouping, range, computer selection, project context, tagging, pagination and project administration have substantial implementations.
- Stable row keys, delegated actions and state-owned drafts support the shared renderer.

**What fails**

- **1280, both themes; also evident at 1536:** the extra project rail compresses account names and project labels. The initial running rows are overwhelmingly local; remote work sits below them. C’s Overview does a better job of representing every computer immediately.
- Filters live in `state.a.sessions`, not the URL. The inventory’s bookmarkable filters and Back behavior are absent.
- Running now can be folded through the generic group renderer, contrary to the explicit inventory requirement.
- New session provides a recent-folder dropdown and text describing “Choose folder…,” but no such action or arbitrary-folder entry (`pages-a.js:348`).
- Filtering is limited to the generated row fields. The prototype does not demonstrate the indexed conversation-search contract; that integration cannot be inferred from the search hint.

**Must change**

Preserve filter state in navigation, keep Running now unfolded, provide the folder action and retain readable account/project identity at 1280. Represent remote work early without making users first discover a computer filter.

## Session — 7/10

**What works**

- The supplied captures give conversation content the main workspace. Changes and Failed are clear, nearby controls.
- Step expansion, diffs, grouped replies, long-run disclosure and Markdown download are implemented.
- Follow state and expanded steps are stored outside the DOM. The resume-here dialog preserves the important limitations about files, attachments and shells.

**What fails**

- **All widths/themes:** the remote branch precedes the archived branch. A remote archived session can offer Resume here instead of the required read-only treatment (`pages-a.js:277–279`).
- Both live timers stop when motion is Still. Turning off animation therefore also stops simulated data arrival, although Still should affect presentation.
- Terminal selection lives in `state.a.terminal`, separately from Settings.
- Session failure counts come from generated transcript steps, while list failure marks come from another field. A failed row can open a transcript reporting no failures.
- The custom Markdown renderer handles a narrow subset without identifying that limit.

**Must change**

Apply archived/resumability rules before constructing actions. Share terminal preferences and session facts with the list. Keep data updates independent of motion. Preserve ordinary Markdown content using the application’s existing rendering approach.

## Projects — 7/10

**What works**

- The finding, live count, distribution and project cards answer useful questions. Sparklines are derived from the session fixture rather than ornamental seeds.
- Pin, rename, tag, merge, local launch and paging are present.
- The 1280 light capture remains legible, and the larger captures retain C’s panel language.

**What fails**

- **1280:** the five readouts and distribution consume most of the fold before the owner’s frequent task—finding and pinning projects—begins.
- Pinned-project navigation is handled only in `mount()`. Clicking a shell pin while already on Projects sets `projectId`; rendering returns “Opening…,” but the same-page mount does not run again (`pages-a.js:291, 479`; `modals.js:754`).
- Range, sort, metric and page choices have no URL representation.
- Merge moves sessions but does not reconcile the source project’s tags and pin entries. Reset-name handling likewise leaves shell pin names separate.
- The shell’s project count is hardcoded. It cannot explain its relationship to the selected range or respond reliably to project edits.

**Must change**

Handle project navigation independently of page entry. Preserve URL state and reconcile aliases, merges and pins through one project model. Compress or fold the distribution so project cards and pin actions arrive earlier.

## MCP servers — 8/10

**What works**

- “11 servers shared. 2 entries left alone.” is an effective finding.
- The supplied captures preserve readable tool columns, transport, connection and actions in both themes. Form/JSON editing, masked values, validation, takeover and removal are implemented.
- The distinction between managed entries and handwritten entries survives inspection.

**What fails**

- The editor is a separate native dialog rather than the specified add/edit route. Its preview banner is honest, but its navigation and modal behavior differ from the base and A.
- A row’s unchecked box does not distinguish “not written” from “left alone”; the user must open Info to locate the exception named in the headline.
- Freshness is the literal “checked 3m ago.” It does not follow import or save state.
- A small server list still has a constrained inner scroller inside the scrolling page, particularly noticeable at 1280.

**Must change**

Keep this layout. Expose the conflicting cells directly, derive synchronization status and freshness, and use the shared editor/navigation contract.

## Skills — 8/10

**What works**

- The managing-needed finding is useful. “258 in store · 255 managed · 3 not managed” explains the counts rather than concealing their difference.
- Source filtering, paging, bulk selection, unmanaged adoption, discovery targets and copy-replacement confirmation retain considerable inventory breadth.
- The supplied 1280 captures keep pagination accessible. Shared tokens and official tool icons fit C.

**What fails**

- `archify` has source Store but receives an Update action. The inventory permits source updates for GitHub and local-path sources; this fixture demonstrates an unsupported combination.
- In Discover, filtering a repository down to no matching skills produces “No SKILL.md in this repository.” Search failure becomes a false claim about repository contents (`pages-b.js:87`).
- The repeated generated descriptions add row height without helping a user distinguish skills.

**Must change**

Gate updates by supported source, distinguish no matches from an empty repository, and use informative representative descriptions. Retain the matrix and management flow.

## System prompts — 6/10

**What works**

- Library versus live file is a sound structure. Tool tabs, destination path and enabled state are understandable in the supplied captures.
- Switching prompts preserves previous live text as a library entry.
- Deleting a library prompt correctly leaves the live file alone.

**What fails**

- **All widths/themes, code-confirmed:** editing and saving an enabled prompt updates its library body but leaves `prompt_live` unchanged. It remains labelled Enabled and “in the file now.” The code-only check reproduced this contradiction.
- Use this prompt is hidden for that enabled entry, leaving no clear way to apply its revised body.
- The head explains which filename a tool reads rather than reporting the selected prompt or a difference requiring action.
- Empty state still displays Live pills despite saying no profile/file exists.

**Must change**

Redraw the saved/edited/live states around actual text equality. Saving an active prompt must either update the live file or explicitly expose an unapplied revision and its apply action. Never label an absent file Live.

## API keys & routing — 6/10

**What works**

- Router status, request counts and the paused provider form a useful operational hierarchy.
- Provider chains, breaker states, logs, validation and destructive confirmations preserve the inventory.
- Mode explanations are disclosed rather than imposed as paragraphs.

**What fails**

- Routes created here do not populate Add account’s route selector. The advertised route → routed-account workflow crosses incompatible data stores and ports.
- Stopping the router leaves the first failover provider labelled Serving: the label depends on its chain position, not router operation (`pages-b.js:91`).
- “Paused · retry 4m” and “Open · 5 failures” are literal strings rather than readings of their fields.
- Removing a route deletes its `routed_profiles` associations from this page instead of resolving the dependent accounts.

**Must change**

Use the same routes, port and dependent accounts as Add account and account settings. Derive serving, retry and breaker labels. Make route removal explicitly handle accounts that use it.

## Apps — 7/10

**What works**

- The update finding and installed/not-installed groups answer the page’s main question immediately.
- Duplicate installations are prominent, and removal identifies which copy stays.
- Show controls, install methods, update commands and the self-update destination retain useful inventory.

**What fails**

- **First run, all widths/themes:** `fresh()` clears every installed app for both empty and first-run presets. The base simultaneously lists detected main profiles.
- A failed or absent newest-version reading can still yield “Up to date,” because that label follows absence from the updates list.
- Update detection compares version inequality, which cannot distinguish an available upgrade from a newer installed version.
- Installing a fixture changes B’s app model but leaves the lead’s launch-capability model unchanged.

**Must change**

Separate first-run detection from no-tools empty state. Represent unknown and failed version checks explicitly, compare versions correctly, and propagate installation state to account preparation and launch eligibility.

## CLI reference — 8/10

**What works**

- All seven inventory groups and the four starting examples are present.
- Commands use monospace while explanations retain body type. The supplied width/theme captures are orderly.
- The page remains a reference without unnecessary interaction or fabricated activity.

**What fails**

- “Every desktop action has an hrm command” is a broad promise rather than a useful reference heading.
- In the 1280 layout, command descriptions wrap while lower groups require substantial scrolling. There is no compact group navigation.
- Windows paths and Credential Manager wording are unconditional despite the app’s macOS audience.

**Must change**

Use a direct reference heading, add compact links to the existing groups, and qualify platform-specific examples. Preserve the current content and restrained layout.

## Settings — 6/10

**What works**

- Section navigation, disclosures, explicit Save buttons for typed values and keyboard-accessible help are practical.
- Computers, pairing, hub recovery, backups, archive and restore have substantial coverage.
- Confirmation text preserves key safeguards: retained history, protected credentials and a backup before restoration.

**What fails**

- Preferences have competing owners. Settings changes terminal selection separately from Session; account detail hardcodes “Live limits off”; studio changes can disagree with Settings’ selected theme, motion and quota display.
- The Computers shortcut is consumed only on Settings entry. Clicking the topbar computer chip while already on Settings does not reliably jump there.
- Fresh state clears backup lists but retains a populated restore preview containing 87 sessions. Choosing an arbitrary folder can therefore present unrelated history.
- Renaming or forgetting computers updates B’s model while the shell and other pages retain their original computer identities and counts.
- The heading asserts locality rather than summarizing relevant state. Section counts are literals, not consistently counts of their visible settings.

**Must change**

Make settings and computer state authoritative across the app. Handle section navigation on every request. Bind restore previews to the selected source and show unread/empty states until that source is inspected.

## Across the app

- **Is it one app?** Visually, substantially yes; operationally, not yet. C’s rail, palette, panel surfaces and compact type survive. A introduces a second project rail, custom tag/menu treatments and a separate modal controller. B uses taller matrices, larger form spacing and document-level native dialogs. The lead retains denser inline-styled administration surfaces. Unify shared components, spacing and dialog ownership without flattening the page-specific task layouts.

- **The owner’s words:** Overview, Sessions, Projects, MCP, Skills, routing and Apps often deliver findings. Account detail and Session appropriately retain identifying titles, but need trustworthy state nearby. Prompts, CLI and Settings lean toward general claims. Accounts retains an explanation sentence. None needs more introductory copy.

- **Buildability:** The registry, classic scripts, CSS tokens and reconciler are viable for one `app/ui` codebase. The smallest complete implementation shares existing account, session, project, route and settings data; it does not preserve the three prototype state stores. A’s comments about keyed state and timer cleanup are supported by code, but its Still mode also freezes updates. B’s external-dialog draft preservation is plausible, but introduces a different modal lifecycle. Phase one’s report substantiates a repaired base; its 84 captures predate the completed extension files and do not establish integrated acceptance.

- **Inventory:** Breadth is good, but controls alone do not preserve a capability. Missing URL navigation, arbitrary-folder selection, complete account history, coherent terminal preferences and route-to-account wiring are concrete omissions. Unsupported API-key cap wording is an untagged invention.

- **Three worst defects, ranked:**
  1. Overview replaces exhausted operational state with unread onboarding and hides running work.
  2. Separate data stores break cross-page tasks: account history, route selection, installed tools, computer identity and preferences.
  3. Prompt editing claims text is enabled in the live file when the file still contains different text.

- **Ranking of the twelve non-Overview pages, best to worst:** CLI reference > MCP servers > Skills > Projects > Session > Apps > Accounts > Settings > Sessions > API keys & routing > System prompts > Account detail.

Checks not performed: browser clicks, fresh rendering, live motion, native tray, WebView2/WKWebView or backend integration; no files were changed.  
Remaining risk: attractive populated captures conceal contradictory unavailable, edited and cross-page states.
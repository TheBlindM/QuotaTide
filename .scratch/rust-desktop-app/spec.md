Status: ready-for-agent
Type: spec
Source: ./map.md

# QuotaTide v1 产品与实施规范

## Problem Statement

共享 Codex 账号的成员无法直观看到当前账号在本次七日额度窗口内已经使用了
多少、每天使用是否符合约定、何时可能重置，也无法在额度接近阈值或第三方
出现高概率重置预测时及时收到提醒。

现有 Node/Docker 原型只能作为行为参考：它依赖本地网页和明文配置，缺少面向
普通用户的安装体验、系统托盘生命周期、原生通知、凭证保护、可靠恢复、双语
与辅助技术支持，也不能作为准备开源的 macOS/Windows 桌面产品直接发布。

用户需要一个只监控当前账号、不会代理请求或修改凭证、常驻系统托盘且可在
本机完成配置和提醒的独立开源应用。它必须严格区分 OpenAI 账号额度事实、
QuotaTide 的每日使用策略和 Codex Resets 的第三方预测，避免把预测当成真实
重置。

## Solution

构建 QuotaTide：一个以 Rust 为核心、使用 Tauri 2 桌面壳和轻量本地 WebView
界面的单账号托盘应用。

应用由用户选择 Codex 软件自动维护的 `auth.json`，每次刷新都重新以只读方式
打开它，通过当前 access token 和 Account ID 查询账号真实七日额度窗口。
QuotaTide 每小时采集一次，将不可变观测、策略版本、每日账本、来源健康和
提醒 outbox 保存到本机 SQLite；SMTP 密码只进入 macOS Keychain 或 Windows
Credential Manager。

紧凑窗口以当前账号的当前七日额度窗口为主要结构，展示周额度、今日实际上限、
每日用量、来源健康和 Codex Resets 预测。默认七日策略模板为
`16/16/16/16/16/10/10`，可由用户逐日修改；工作日结转开启时，已确认的未用
工作日额度平均分给同一自然周内后续工作日，不进入周末，也不跨自然周。

应用默认通过系统通知提醒，支持配置一个发件邮箱和多个收件邮箱。通知和邮件
共享持久化提醒事件，但分别投递、分别重试。所有运行数据留在本机，默认无
遥测，诊断导出经过 allowlist 脱敏。

v1 以 MIT License 开源，产品名为 QuotaTide，作者为 TheBlind，永久应用标识
为 `dev.theblind.quotatide`。首发采用未签名 `0.x` 预览版：macOS universal
DMG 和 Windows x64 per-user NSIS；Tauri updater signature 仍强制验证。
正式候选最低系统版本为 macOS 15 Sequoia（Apple Silicon 与 Intel）和
Windows 11 25H2 x64。

## User Stories

1. As a shared Codex account member, I want to see the current account’s weekly usage and remaining quota, so that I can decide whether to start a large task.
2. As a shared Codex account member, I want the dashboard to show the current seven-day quota window rather than a rolling recent-seven-days chart, so that the dates match the account’s real reset cycle.
3. As a shared Codex account member, I want each day in the current window to show usage, limit, and state, so that I can understand how the weekly quota was consumed.
4. As a shared Codex account member, I want unknown days to remain visibly unknown, so that missing observations are not mistaken for zero usage.
5. As a shared Codex account member, I want to see today’s base quota, carried quota, and actual limit, so that the daily recommendation is explainable.
6. As a shared Codex account member, I want unused confirmed weekday quota divided among later weekdays, so that the team can safely reuse capacity.
7. As a shared Codex account member, I want weekday overuse not to reduce later days’ base quota, so that one heavy day does not silently rewrite the agreed policy.
8. As a shared Codex account member, I want carry to exclude weekends and later natural weeks, so that the weekly policy remains predictable.
9. As a shared Codex account member, I want policy changes to affect today and future dates without rewriting completed history, so that old reports remain auditable.
10. As a shared Codex account member, I want daily and weekly threshold states to be advisory only, so that QuotaTide never blocks Codex usage.
11. As a user, I want to choose my `auth.json` with the native file picker, so that setup does not require editing environment variables.
12. As a user, I want QuotaTide to treat `auth.json` as strictly read-only, so that it cannot interfere with Codex authentication.
13. As a user, I want every refresh to reopen `auth.json`, so that token rotation performed by Codex is picked up automatically.
14. As a user, I want invalid, unreadable, or replaced auth files to produce safe actionable errors, so that I can repair setup without exposing credentials.
15. As a user, I want an account change in the selected auth file to create a separate account stream, so that histories from different accounts are never merged.
16. As a user, I want only the current account shown in the interface, so that v1 stays simple even if old account streams remain locally preserved.
17. As a user, I want QuotaTide to refresh once at startup and then hourly, so that data remains useful without excessive polling.
18. As a user, I want a manual refresh action with a 30-second cooldown, so that I can check immediately without creating request bursts.
19. As a user, I want concurrent refresh triggers merged into one request cycle, so that startup, timer, and manual actions cannot duplicate work.
20. As a user, I want successful Codex and Radar results handled independently, so that one failed source does not erase the other source’s last known good data.
21. As a user, I want stale data clearly labeled with the last successful refresh time, so that I do not mistake old values for live values.
22. As a user, I want QuotaTide to identify only a strict 604800-second account window, so that unrelated upstream rate-limit windows are ignored.
23. As a user, I want quota usage to use a persistent high-water mark within an epoch, so that minor upstream regressions do not rewrite history.
24. As a user, I want a new quota epoch created only from confirmed account evidence, so that reset text drift or a third-party post cannot fabricate a reset.
25. As a user, I want current Reset Radar probability, validity period, explanation, and source link shown separately, so that I can judge the third-party estimate.
26. As a user, I want expired or malformed Radar predictions hidden, so that stale estimates are not presented as current.
27. As a user, I want the Radar probability displayed using the source site’s bucket semantics, so that QuotaTide does not claim false precision.
28. As a user, I want Radar failure to leave account quota monitoring operational, so that the optional prediction source is never a core dependency.
29. As a user, I want system notifications when configured thresholds are crossed, so that I can react without keeping the window open.
30. As a user, I want notifications deduplicated by date, quota epoch, prediction window, and threshold as appropriate, so that hourly refreshes do not spam me.
31. As a user, I want clicking a notification to open the existing tray window at the relevant status, so that I can understand the alert immediately.
32. As a user, I want denied or failed system notifications reflected in the app while email continues independently, so that one channel cannot swallow an event.
33. As a user, I want to configure a sending address and multiple recipient addresses, so that the shared account’s members can all receive selected alerts.
34. As a user, I want to send a test email after configuring SMTP, so that I can verify the configuration before relying on alerts.
35. As a security-conscious user, I want SMTP to require TLS relay or required STARTTLS, so that credentials and messages are never sent over plaintext SMTP.
36. As a security-conscious user, I want the SMTP password stored only in the operating system credential vault, so that it never appears in SQLite or normal config.
37. As a user, I want changing the SMTP password, autostart, and ordinary settings to commit atomically, so that partial saves cannot leave contradictory state.
38. As a user, I want a missing, locked, or denied credential vault to pause email without deleting configuration, so that I can recover after unlocking it.
39. As a user, I want alert events and channel deliveries persisted before sending, so that an application crash does not lose alerts.
40. As a user, I want transient delivery failures retried with backoff and stable idempotency, so that recovery does not create duplicate alerts.
41. As a user, I want QuotaTide to start at login only when I enable it, so that background startup remains under my control.
42. As a user, I want login startup to create only the tray and scheduler without showing the window, so that sign-in remains quiet.
43. As a user, I want the compact window anchored to the tray icon and kept on screen across displays and scaling modes, so that it never opens out of reach.
44. As a user, I want left-click to toggle the window and right-click to show a small native menu, so that the tray behavior matches desktop conventions.
45. As a user, I want closing or losing focus to hide the window while an explicit Quit action ends the process, so that the app behaves like a tray utility.
46. As a user, I want native dialogs and permission prompts protected from automatic focus-loss hiding, so that setup actions are not interrupted.
47. As a user, I want Apple-style native material where supported, so that the compact utility feels integrated with macOS and Windows.
48. As a user, I want an opaque high-contrast fallback whenever native material fails or transparency is disabled, so that visual effects never block functionality.
49. As a user, I want the interface to follow light and dark appearance, so that it remains comfortable with my system settings.
50. As a Chinese-speaking user, I want the entire product available in Simplified Chinese, so that setup, errors, notifications, and email are understandable.
51. As an English-speaking user, I want the entire product available in English, so that no core workflow depends on Chinese text.
52. As a user, I want interface language, formatting region, and policy timezone treated separately, so that changing presentation cannot alter quota accounting.
53. As a user, I want percentages, reset times, and countdowns formatted with my regional conventions, so that values are easy to read.
54. As a user, I want queued notifications and emails to retain the language and timezone snapshot from event creation, so that retries do not change message meaning.
55. As a keyboard user, I want every core workflow reachable without a pointer, so that I can configure and operate the tray window efficiently.
56. As a VoiceOver user, I want headings, status, progress, charts, controls, and validation errors announced correctly, so that I can use the macOS app independently.
57. As a Narrator user, I want the same core workflows available with the display off, so that the Windows app is independently usable.
58. As a low-vision user, I want WCAG 2.2 AA contrast, visible focus, large icon targets, and 200% text support, so that the compact window remains operable.
59. As a motion-sensitive user, I want reduced motion to remove nonessential animation, so that status changes remain comfortable and immediate.
60. As a user who enables reduced transparency or forced colors, I want the glass effect fully removed and system colors respected, so that content stays readable.
61. As a user, I want local state to survive restart, crash, and normal upgrades, so that usage history and pending alerts are reliable.
62. As a user, I want migrations backed up and verified before changing my database, so that an upgrade cannot silently destroy history.
63. As a user, I want corrupted state restored from the latest valid backup when possible, so that recovery is automatic but auditable.
64. As a user, I want a dedicated recovery interface when all backups are invalid, so that QuotaTide never creates an empty database and pretends nothing happened.
65. As a privacy-conscious user, I want all account facts, policy, recipients, and logs to remain on my device, so that using the monitor does not create a cloud profile.
66. As a privacy-conscious user, I want no telemetry or automatic crash upload, so that diagnostic sharing is always explicit.
67. As a support-seeking user, I want to export a strictly allowlisted diagnostic archive, so that I can report a problem without exposing tokens, paths, emails, or database contents.
68. As a user, I want “clear all local data” to remove QuotaTide state, vault entries, logs, backups, and autostart without touching `auth.json`, so that removal is complete and scoped.
69. As a user, I want updates checked after startup and then daily, so that I can discover releases without continuous polling.
70. As a user, I want to review release notes and explicitly approve installation and restart, so that QuotaTide never silently updates.
71. As a security-conscious user, I want every update artifact verified by the embedded Tauri updater public key, so that unsigned preview distribution cannot inject an untrusted update.
72. As a user, I want a failed, interrupted, or tampered update to leave the current version usable, so that recovery does not require data loss.
73. As an open-source user, I want the app identity, author, license, source link, privacy statement, and third-party notices to be consistent, so that I can audit what I installed.
74. As an open-source contributor, I want pull-request CI isolated from release secrets, so that contributions cannot access signing or publication credentials.
75. As a maintainer, I want immutable release artifacts and higher-patch roll-forward recovery, so that a published binary is never silently replaced.
76. As a maintainer, I want release evidence for every supported OS and CPU combination, so that “supported” means installed and tested rather than merely compiled.
77. As a maintainer, I want explicit CPU, memory, startup, network, UI bundle, and log budgets, so that a background tray utility remains lightweight.
78. As a user on an unsupported system, I want installation to fail or warn accurately, so that best-effort smoke results are not mistaken for a support promise.

## Implementation Decisions

1. The product identity is QuotaTide, authored by TheBlind, with permanent application identifier `dev.theblind.quotatide` and MIT License.
2. QuotaTide is an independent community application and must not imply endorsement by or affiliation with OpenAI.
3. v1 is a single-account desktop tray application for macOS and Windows; it does not expose a local HTTP service or traditional main window.
4. The desktop stack is Tauri 2 with a Rust-owned application core and a Vite, Preact, and TypeScript interface rendered in the system WebView.
5. The workspace has three architectural areas: a framework-independent quota core, a thin Tauri adapter/composition layer, and a presentation-only UI.
6. The UI receives typed public DTOs and submits typed drafts. It does not read files, access SQLite, call upstream services, manage the tray, send notifications, invoke the updater, or hold secrets.
7. Tauri capabilities use a narrow allowlist; the WebView receives no general filesystem, shell, HTTP, notification, dialog, autostart, updater, SQL, or credential-vault permission.
8. `QuotaLedger` is the primary pure domain seam and owns quota epochs, high-water usage, daily deltas, policy revisions, carry conservation, threshold crossings, and dashboard facts.
9. Quota percentages use integer `QuotaUnits`, with one million units per percentage point and 100% represented exactly; floating-point values are converted once at adapter boundaries.
10. Instants are persisted in UTC, while natural days are calculated with the configured IANA policy timezone.
11. The default seven-day policy is Monday through Sunday `16/16/16/16/16/10/10`; any policy revision must contain seven nonnegative values totaling no more than 100%.
12. Workday carry is optional. It moves only confirmed unused Monday-Friday quota to later workdays in the same natural week, never to weekends or a later natural week.
13. Missing daily observations remain unknown and cannot create carry. Overuse does not create negative carry or reduce later base quota.
14. A policy or timezone edit appends a revision effective today and forward; completed daily ledger rows retain their original policy snapshot.
15. `RefreshCoordinator` is the high-level refresh seam and owns startup/hourly/manual triggers, single-flight behavior, cooldown, auth reread, parallel sources, ledger transition, atomic persistence, and dashboard revision events.
16. The scheduler refreshes immediately after startup and hourly thereafter, skips missed interval bursts after sleep, and merges concurrent triggers. Manual refresh has a 30-second cooldown.
17. `auth.json` is selected through a Rust-owned native dialog, normalized, validated, and reopened read-only for every refresh. QuotaTide never edits, moves, deletes, or changes its permissions.
18. Authentication material is limited to the access token and canonical Account ID required by the Codex usage request. It is wrapped as secret data in Rust and never crosses IPC or persistence boundaries.
19. A 401/403 can retry once only when a fresh disk read proves that the token changed during the request.
20. The Codex adapter uses the authenticated WHAM usage endpoint, required headers, fixed origin, strict timeout, sanitized errors, and a strict 604800-second current-window selector.
21. Account identity changes create or reactivate an isolated account stream. Only the current stream is projected to the dashboard; old streams remain local and are never merged.
22. Within an epoch, usage is monotonic by persisted high-water mark. A new epoch requires confirmed account evidence such as a material usage reset with coherent window facts; reset timestamp drift alone is insufficient.
23. The Reset Radar adapter reads the public Codex Resets API, validates probability and validity timestamps, records source health independently, and displays the source’s bucketed probability semantics.
24. Radar content is third-party evidence only. It cannot create or close a quota epoch, modify account usage, change daily limits, or claim an OpenAI reset.
25. Codex and Radar requests run concurrently with separate last-known-good states and separate timeouts. A partial source failure does not roll back the successful source.
26. `SettingsManager` is the high-level configuration seam and owns path validation, policy validation, SMTP validation, credential-vault staging, autostart synchronization, optimistic revision checks, commit, and rollback.
27. Non-secret settings, immutable facts, current state, projections, source health, alert events, deliveries, attempts, migration metadata, and recovery journals are stored in one versioned SQLite database owned by Rust.
28. SQLite uses one serialized connection thread, foreign keys, WAL, busy timeout, forward-only migrations, immutable fact protections, and bundled SQLite to avoid a hidden system-library floor.
29. Migrations run only after a validated rolling backup. A newer unsupported schema fails safely without downgrade; failed migrations roll back and enter the defined recovery flow.
30. Startup checks directory permissions, SQLite recovery files, integrity, schema, domain invariants, unfinished external-change journals, and rebuildable projections before starting background workers.
31. Corrupted state is isolated, then restored from the newest valid backup and revalidated. If no backup is valid, the app enters a recovery UI rather than creating an empty replacement.
32. SMTP passwords use two fixed application-scoped credential slots in macOS Keychain or Windows Credential Manager. SQLite stores only a credential reference and non-secret mail settings.
33. Secret updates are explicit `Keep`, `Set`, or `Delete` operations. A staged credential is written and read back before SQLite commit; failures restore the old autostart and credential state.
34. SMTP uses a reused asynchronous `lettre` transport with rustls and supports only implicit TLS relay or required STARTTLS. Plaintext and opportunistic downgrade modes are unavailable.
35. One sender configuration can contain multiple active recipient addresses. Each reminder creates independently tracked channel deliveries, allowing partial success without duplicate reminder events.
36. `DeliveryWorker` is the high-level delivery seam. It claims persisted outbox deliveries with leases, records sanitized attempts, applies bounded exponential retry, and preserves channel isolation and idempotency.
37. A test email is an explicit user action using the same validation, TLS, timeout, and sanitization rules, but does not create a quota reminder event.
38. Stable reminder kinds include daily 80% and 100% crossings, weekly remaining 20% and 10% crossings, Radar 70% bucket crossing, confirmed new quota epoch, and three consecutive failures from one source.
39. Reminder deduplication keys include the appropriate account stream, quota epoch, natural day or prediction window, event kind, threshold, and channel.
40. Reminder deliveries snapshot the interface locale, format locale, policy timezone, message key, and structured arguments at event creation. Retry renders from the snapshot.
41. System notification permission is requested only with user context. Denial or failure preserves the reminder and displays an in-app state; email remains independent.
42. Notifications are sent from Rust. Windows notification identity is accepted only from an installed build, not development mode.
43. Autostart is controlled through a Rust adapter and committed atomically with settings. Login launch creates the tray and workers without showing the compact window.
44. The tray shell owns a single fixed-size 420×680 window. Left-click toggles it, right-click opens a native localized menu, close and focus loss hide it, and explicit Quit terminates it.
45. Tray-event geometry, active display bounds, scale factor, and fallback placement keep the window visible. Native dialogs and permission prompts activate a modal guard against focus-loss hiding.
46. macOS runs as an accessory application without a Dock icon; Windows uses a hidden taskbar entry. A failure is visible in diagnostics rather than silently changing lifecycle semantics.
47. macOS independent distribution may use the private transparent-WebView path with Popover or HudWindow material. This makes Mac App Store distribution out of scope for the same build.
48. Windows prefers Acrylic for the transient panel and can fall back to Mica on Windows 11 or a fully opaque surface. Any material failure leaves all product functions available.
49. Reduced transparency, increased contrast, high contrast, and forced-colors signals replace native effects with opaque semantic surfaces. Reduced motion removes all nonessential interpolation and movement.
50. The selected Tide Dial visual identity uses a circular quota gauge and rising tide. Application icons retain seven-day marks at large sizes; tray assets use platform-specific monochrome and high-contrast variants.
51. The interface has two internal states, overview and settings, without a router or global state library. Rust state is authoritative; local UI state is limited to uncommitted drafts.
52. The overview prioritizes the current seven-day window, weekly usage/reset, today’s actual limit, source health, and Reset Radar. Settings are grouped into quota, account, and notifications.
53. The UI uses semantic HTML, CSS, and local SVG without remote fonts, scripts, analytics, or a charting dependency.
54. v1 fully supports `zh-CN` and `en`, with English as final fallback. All QuotaTide-owned UI, errors, tray menus, notifications, emails, recovery flows, installer guidance, diagnostics labels, and about text are localized.
55. Interface language can follow the system or be set explicitly. BCP 47 resolution maps Simplified Chinese inputs to `zh-CN`, English inputs to `en`, and Traditional Chinese or unsupported inputs to English rather than silently converting script.
56. Interface language, system formatting region, and IANA policy timezone are independent inputs. Formatting uses `Intl`; accounting never depends on presentation locale.
57. Percentages show at most one decimal without meaningless `.0`; reset countdowns use minute precision plus an absolute policy-timezone value and never expose invalid-number placeholders.
58. WCAG 2.2 AA is the release gate without claiming third-party certification. Core targets, focus, contrast, keyboard order, screen-reader semantics, chart alternatives, 200% text, and reduced-effects modes follow the accepted accessibility baseline.
59. Public errors contain only a stable code, localization key, and allowlisted safe context. Internal errors retain chains for sanitized local logs but never cross IPC verbatim.
60. Logs rotate at a hard 5 MiB total limit. Diagnostics are generated by reserializing allowlisted DTOs into a restricted temporary directory, scanning forbidden fields, archiving, and deleting temporary data.
61. Token, JWT, cookies, auth contents and path, account IDs, recipient and sender addresses, SMTP host/username/password, credential references, raw upstream bodies, and database files are excluded from logs and diagnostics.
62. “Clear all local data” is explicitly destructive and double-confirmed. It stops workers, deletes both vault slots, clears autostart and QuotaTide-owned state, then recreates an empty app directory without touching `auth.json`.
63. The release support matrix is macOS 15 Sequoia universal on Apple Silicon and Intel, plus Windows 11 25H2 x64. Windows 11 26H1 receives new-hardware compatibility smoke.
64. macOS 14, Windows 10 22H2, and Windows 11 24H2 are best-effort expansion smoke targets only and do not become supported merely by passing once.
65. macOS uses a universal DMG and universal updater archive. Windows uses a current-user x64 NSIS installer with Evergreen WebView2 `embedBootstrapper`; MSI, per-machine install, fixed runtime, ARM64, Linux, mobile, and web are excluded.
66. The initial public channel is an unsigned `0.x` preview distributed through GitHub Releases. Gatekeeper and SmartScreen warnings are documented accurately without instructing users to disable system protection globally.
67. The updater checks after 60 stable seconds, then every 24 hours, with a manual action and an opt-out for automatic checks. It does not predownload, silently install, or force restart.
68. Update manifests are static GitHub Release assets. Both macOS architecture keys point to the same universal archive; every platform entry has an HTTPS URL and Tauri signature.
69. The updater public key is compiled into the app and signature verification cannot be disabled. Private updater keys exist only in a protected release environment and two independent encrypted offline recovery copies.
70. Published tags and assets are immutable. Recovery uses a higher patch release and forward update, never replacement of existing bytes or automatic downgrade.
71. Pull-request CI has no release secrets or write permission. Release publication is isolated behind a protected environment and maintainer approval, with provenance and artifact hashes.
72. Production updater endpoints, repository metadata, security links, and provenance remain behind a hard placeholder gate until the user confirms one GitHub `owner/repo`.
73. The old Node/Docker service is not translated file by file and its runtime data is not migrated. Accepted behavioral contracts are reimplemented in Rust; legacy server, Docker, plaintext-secret, and browser-service code is removed before open source release.
74. Initial resource gates are: production UI gzip at most 100 KiB, hidden idle CPU below 0.5%, app plus dedicated WebView memory target at most 180 MiB, cold start to interactive tray at most 2.5 seconds, hourly normal network cadence, and no per-refresh thread creation.

## Testing Decisions

1. Tests assert public behavior at the highest stable seam and do not couple to private helper functions, SQL statement order, framework internals, or visual implementation details.
2. The primary domain seam is `QuotaLedger`; table-driven and property tests cover epochs, high-water monotonicity, first observations, cross-midnight allocation, DST, policy revisions, carry conservation, unknown days, overuse, and threshold crossings.
3. Carry property tests prove that historical days never change, unknown days cannot create carry, weekends cannot receive carry, later natural weeks cannot receive carry, and the seven-day base total never exceeds 100%.
4. The refresh seam is `RefreshCoordinator`; integration tests use a fake clock, in-memory implementations of the source ports, and the real SQLite store to cover startup/hourly/manual triggers, cooldown, single-flight, token-change retry, partial source failure, and last-known-good projection.
5. Upstream adapter contract tests use committed sanitized fixtures for valid and malformed auth structures, account identity, exact WHAM window selection, missing fields, rate-limit responses, reset drift, Radar validity, bucket semantics, timeout, and sanitized errors.
6. No default CI test accesses a real Codex account. Real-account validation is explicit, ignored/manual, uses a user-selected auth file, and forbids secrets in fixtures, snapshots, command output, and artifacts.
7. SQLite tests use the same production store with memory or temporary disk databases. They cover schema creation, every migration path, checksums, uniqueness, append-only protections, transaction rollback, WAL restart, projection rebuild, and newer-schema refusal.
8. Recovery tests corrupt the main database and each rolling backup in controlled combinations, proving isolation, newest-valid restoration, invariant checking, recovery UI entry, and no silent empty-state creation.
9. Settings tests exercise optimistic revisions, validation failures, `Keep/Set/Delete`, every two-phase journal crash point, credential read-back failure, autostart failure, SQLite commit failure, and rollback convergence.
10. Delivery tests use recording notification and SMTP adapters to verify outbox atomicity, lease expiry, channel isolation, per-recipient results, retry classes, idempotency, language snapshots, and safe errors.
11. SMTP contract tests include implicit TLS, required STARTTLS, invalid certificate, unavailable upgrade, timeout, transient server errors, permanent authentication errors, and multiple-recipient partial failure.
12. Tauri command tests verify the capability allowlist, DTO generation, revision events, safe error shape, absence of secrets, and inability of the WebView to invoke platform primitives directly.
13. UI tests use Testing Library and accessibility tooling against overview and settings states, including fresh, warning, exceeded, stale, empty, validation error, delivery error, recovery, and update-available states.
14. Locale tests compare the complete `zh-CN` and `en` key sets and cover supported, unsupported, invalid, Traditional Chinese, region, DST, plural, relative-time, percentage, and queued-message snapshot cases.
15. Keyboard integration tests cover opening, overview, all settings groups, file selection return, policy editing, recipient chips, SMTP test, validation recovery, save, Escape, manual refresh, and close.
16. Automated accessibility checks must have no critical or serious violations. Pseudo-localization expands content by approximately 40% and is tested at 420px width and 200% text size.
17. VoiceOver on macOS and Narrator on Windows must complete the same core workflows without a pointer; visual assistance tests cover light/dark, contrast, forced colors, reduced transparency, reduced motion, and combinations.
18. Platform behavior is accepted only from installed release artifacts. Development mode is not evidence for Windows notification identity, installers, autostart, updater, credential vault, or uninstall behavior.
19. Every release blocks on installed smoke for macOS 15 Apple Silicon, macOS 15 Intel, Windows 11 25H2 x64, current macOS Apple Silicon, and Windows 11 26H1 compatibility.
20. Installed smoke covers CPU slices, first install, single instance, upgrade, uninstall/reinstall, tray clicks, positioning, focus lifecycle, sleep recovery, materials and fallback, notifications, autostart, dialog, credential vault, SMTP, SQLite recovery, updater, and data clearing.
21. Windows WebView2 testing covers existing Evergreen runtime, a clean environment using the embedded bootstrapper, bootstrapper failure, and a later runtime update. No minimum WebView2 version is invented before implementation audit.
22. Release evidence identifies version, commit, artifact hash, OS/build, CPU, WebView2 version, test ID, result, executor, timestamp, evidence path, and linked defect. Required states are `PASS`, `FAIL`, `BLOCKED`, or `N/A`; unfinished cells cannot release.
23. Security tests seed canary token, path, account ID, email, SMTP username/password, and upstream error values, then recursively scan public DTOs, logs, diagnostics, snapshots, and artifacts.
24. Network tests verify that the Rust process contacts only the fixed Codex and Radar origins, the user-configured SMTP server, and the configured GitHub updater endpoint, at the approved cadence and without sensitive updater parameters.
25. Resource measurements use final release artifacts without debugger or development server. CPU and memory include the complete QuotaTide process tree and are measured on the minimum supported macOS CPU variants and Windows environment.
26. UI bundle size is checked automatically in CI. Idle CPU is measured after five hidden stable minutes, memory includes the dedicated WebView, cold startup is measured from process creation to clickable tray over five runs, and log rotation is driven past its cap.
27. A resource budget can change only through an ADR containing measurements from both supported platforms; tests cannot be silently removed or thresholds raised for convenience.
28. Release tests verify version consistency, architecture, deployment target, identifier, icons, license/notices, checksums, updater signatures, immutable assets, release provenance, and public-download reinstallation.
29. Roll-forward drills prove that a bad release is superseded by a higher patch version and that tampered, incomplete, canceled, offline, or disk-full update attempts leave the installed version usable.
30. The GitHub placeholder gate fails release builds until the confirmed `owner/repo`, endpoint, metadata links, protected environment, security reporting, provenance subject, and public manifest reachability are all bound.

## Out of Scope

- Multiple concurrently monitored Codex accounts, account lists, teams, roles, user login, permissions, or cloud synchronization.
- Per-person or per-request usage attribution for a shared account.
- Proxying, intercepting, throttling, blocking, or automatically stopping Codex requests.
- Modifying, refreshing, moving, deleting, or changing permissions on `auth.json`.
- Calling or automatically using banked-reset endpoints.
- Treating Codex Resets predictions or posts as proof of an account reset.
- Automatic public-holiday, make-up-workday, or organization-calendar logic.
- A hosted backend, public dashboard, LAN service, web edition, mobile edition, or Linux package.
- Importing the Node/Docker prototype database or preserving its runtime deployment model.
- Mac App Store or Microsoft Store distribution, MSI, enterprise per-machine deployment, Windows ARM64, or fixed WebView2 runtime.
- Paid Apple Developer ID, notarization, Windows Authenticode, or guaranteed SmartScreen reputation for the initial unsigned preview.
- Automatic update installation, silent restart, forced update, downgrade, beta-channel selector, custom update server, or CDN.
- Telemetry, advertising, analytics, cloud crash reporting, automatic diagnostic upload, or automatic email attachment of diagnostics.
- Arbitrary SQLite import, raw database export, raw log export, credential export, or secret recovery.
- HTML loaded from remote origins, remote scripts, remote fonts, plugins downloaded at runtime, or a general browser.
- A guaranteed support promise for macOS 14, Windows 10 22H2, Windows 11 24H2, or any platform outside the formal support matrix.

## Further Notes

- The public repository is intentionally unresolved. A release must fail while any GitHub
  owner, repository, updater, security, about, or provenance value is still a placeholder.
- The first public version is `0.1.0`. Ordinary `0.x` preview releases use the stable updater
  channel; release candidates are prereleases and are excluded from the latest manifest.
- Unsigned preview documentation may describe only operating-system-provided per-app
  exceptions. It must not instruct users to disable Gatekeeper, SmartScreen, Smart App
  Control, antivirus, certificate validation, or other system-wide protections.
- Formal platform compatibility remains provisional until the same final artifacts pass the
  release QA matrix. A successful compile or development run is not a support claim.
- The current standalone service specification remains useful only as an earlier behavioral
  source. Where it conflicts with this spec’s desktop architecture, credential storage,
  quota-epoch confirmation, security, localization, distribution, or recovery decisions,
  this spec and its linked research decisions take precedence.
- Implementation should cite release-gate IDs from the minimum-system and release-QA
  decision when tickets are generated, so that platform work cannot be closed with a generic
  “works on macOS/Windows” statement.

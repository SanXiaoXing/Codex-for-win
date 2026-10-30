# CodexBar Windows MVP

A Windows tray app that shows Codex subscription quota, WorkBuddy credits, TRAE token usage, and AtomCode CodingPlan usage.

## Requirements

- Windows 10 or 11
- Node.js, pnpm, Rust stable, and the Visual Studio C++ build tools

## Run

```powershell
npm install
npm run tauri -- dev
```

Codex usage is read from `%USERPROFILE%\.codex\auth.json` (or `%CODEX_HOME%\auth.json`). Sign in with Codex CLI first.

For WorkBuddy, sign in at `workbuddy.cn` in Chrome. In DevTools Network, select a request to `/profile/plans-usage`, then copy its `Cookie` and `User-Agent` request headers into Settings. The session is stored in the Windows Credential Manager. If the session expires or returns HTTP 401, copy fresh headers from the same browser request.

For TRAE personal usage, open [the TRAE usage dashboard](https://www.trae.cn/dashboard#usage) while signed in, then use DevTools Network to pick any request going to `api.trae.cn` and copy its `Cookie` (and optionally `User-Agent`) into Settings. CodexBar exchanges that browser session for a short-lived JWT through `POST /cloudide/api/v3/common/GetUserToken` and then queries TRAE's dashboard endpoint, so the token refreshes automatically and does not have to be re-pasted.

Pasting the raw `Authorization` header (find a `query_user_usage` request and copy its complete value) still works as a fallback, but that JWT is only valid for 8 hours. The longer-lived credentials TRAE issues are not usable here: the 7-day JWT in `%USERPROFILE%\.trae-cn\trae-jwt-token` is rejected by the billing endpoints with `401 code 1001`, and the 166-day OAuth `RefreshToken` is bound to a device key pair stored in TRAE's encrypted local store. Both endpoints are undocumented personal-account interfaces and may change; TRAE Enterprise provides a separately documented admin OpenAPI.

AtomCode needs no credential pasting: the tab reads the CLI's own login state from `%USERPROFILE%\.atomcode\auth.toml` and calls `GET https://api.atomgit.com/api/v5/coding-plan/status-v2` with `Authorization: Bearer <access_token>`. It shows the CodingPlan rolling-window usage (used percent plus window reset time) and the plan validity (`expires_at` / `remaining_days`). It also performs the daily CodingPlan check-in for you: `POST /coding-plan/claim-v2` with the `plan_type` reported by `status-v2`, at most once per day and immediately again if the plan has lapsed. The outcome is appended to the account line (`今日已签到` / `签到成功` / `签到失败`). If the login state expires, run `atomcode login` once.

The app refreshes every five minutes. The tray icon shows Codex's 5-hour usage percentage; hover it to see Codex, WorkBuddy, TRAE, and AtomCode usage. Use the tray icon's context menu to open the panel, refresh, or quit.

In floating-ball mode, the displayed quota follows the foreground Windows app within about one second: Codex, WorkBuddy, TRAE (including TRAE CN), or AtomCode. Codex's Windows package is also recognized when its executable is named `ChatGPT.exe`; ordinary ChatGPT is not treated as Codex. Switching to an unrelated app or clicking the ball keeps the last provider. The expanded panel keeps manual tab selection. Detection uses executable paths, so CLI sessions inside terminals, editor extensions, and browser tabs are not identified. Switching providers displays their latest synced quota; usage still refreshes every five minutes.

AtomCode connections prefer IPv6 when DNS returns both address families, with IPv4 as a fallback. This avoids TLS handshake failures on networks where the AtomGit IPv4 route is broken. To check both read-only usage endpoints with your local CLI login, run `cargo test --manifest-path src-tauri/Cargo.toml live_atomcode_status_and_usage_connect -- --ignored`.

## Build

```powershell
npm run tauri -- build
```

## Website downloads

The static page in `site/` reads the latest published GitHub Release from `SanXiaoXing/Codex-for-win`. Publish each stable version with its tag and attach a Windows x64 NSIS installer whose name contains `CodexBar`, `x64` (or `amd64`), and ends in `setup.exe`. The page displays that tag and links directly to the matching Release asset. The HTML also embeds the last known working installer and version, so downloads still work if the GitHub API is unavailable; update that fallback when publishing the site with a new version.

## Frontend structure

- `src/domain`: shared usage types and provider metadata.
- `src/features/usage`: Codex, WorkBuddy, and TRAE usage views.
- `src/features/floating`: floating usage indicator.
- `src/features/settings`: credentials and appearance dialog.
- `src/features/shell`: app toolbar, provider switcher, and context menu.
- `src/hooks`: usage loading, floating window behavior, and appearance preferences.
- `src/components`: small shared UI pieces.
- `src/lib`: formatting and derived usage values.
- `src/styles`: tokens and feature-specific styles; `src/styles.css` gathers them.


## Rust structure

- `src-tauri/src/app.rs`: Tauri builder, managed state, and command registration.
- `src-tauri/src/model.rs`: usage data types and initial state.
- `src-tauri/src/commands.rs`: frontend-facing Tauri commands.
- `src-tauri/src/providers/`: Codex, WorkBuddy, and TRAE requests plus refresh coordination.
- `src-tauri/src/credentials.rs`: Windows Credential Manager storage for provider credentials.
- `src-tauri/src/window.rs`: Acrylic setup, floating-window sizing, and native window transitions.
- `src-tauri/src/tray.rs`: tray menu, events, and tooltip text.

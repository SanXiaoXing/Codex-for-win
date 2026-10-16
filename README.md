# CodexBar Windows MVP

A Windows tray app that shows Codex subscription quota, WorkBuddy credits, and TRAE token usage.

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

The app refreshes every five minutes. The tray icon shows Codex's 5-hour usage percentage; hover it to see Codex, WorkBuddy, and today's TRAE usage. Use the tray icon's context menu to open the panel, refresh, or quit.

## Build

```powershell
npm run tauri -- build
```

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

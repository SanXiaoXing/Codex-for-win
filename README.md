# CodexBar Windows MVP

A Windows tray app that shows Codex subscription quota and WorkBuddy monthly credits.

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

The app refreshes every five minutes. The tray icon shows Codex's 5-hour usage percentage; hover it to see both Codex windows and WorkBuddy's remaining credits. Use the tray icon's context menu to open the panel, refresh, or quit.

## Build

```powershell
npm run tauri -- build
```

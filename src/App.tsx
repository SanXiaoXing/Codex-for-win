import { useCallback, useEffect, useRef, useState } from "react";
import { useUsageData } from "./hooks/useUsageData";
import { useFloatingMode } from "./hooks/useFloatingMode";
import { useAppearancePreferences } from "./hooks/useAppearancePreferences";
import { SettingsDialog } from "./features/settings/SettingsDialog";
import { ContextMenu } from "./features/shell/ContextMenu";
import { AppToolbar } from "./features/shell/AppToolbar";
import { ProviderTabs } from "./features/shell/ProviderTabs";
import type { PointerEvent as ReactPointerEvent } from "react";
import type { TabKey } from "./domain/usage";
import { shortTime, ballInfo } from "./lib/usage-format";
import { BallVisual } from "./features/floating/BallVisual";
import { CodexUsage } from "./features/usage/CodexUsage";
import { WorkBuddyUsage } from "./features/usage/WorkBuddyUsage";
import { TraeUsage } from "./features/usage/TraeUsage";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";


export default function App() {
  const [tab, setTab] = useState<TabKey>("codex");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const openSettings = useCallback(() => setSettingsOpen(true), []);
  const { usage, loading, refresh } = useUsageData(openSettings);
  const { preferences, setPreferences } = useAppearancePreferences();
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const reduceMotion = !!preferences.motion || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  const { ball, modeTransition, enterBall, exitBall } = useFloatingMode(reduceMotion);
  const [cookie, setCookie] = useState("");
  const [userAgent, setUserAgent] = useState("");
  const [traeAuthorization, setTraeAuthorization] = useState("");
  const [traeCookie, setTraeCookie] = useState("");
  const [traeUserAgent, setTraeUserAgent] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const ballDrag = useRef({ pointerId: -1, startX: 0, startY: 0, dragging: false });

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") { setSettingsOpen(false); setMenu(null); return; }
      if (!(event.ctrlKey || event.metaKey)) return;
      if (event.key === ",") { event.preventDefault(); setSettingsOpen(true); }
      if (settingsOpen || /INPUT|TEXTAREA/.test((event.target as HTMLElement).tagName)) return;
      if (event.key.toLowerCase() === "r") { event.preventDefault(); void refresh(); }
      if (["1", "2", "3"].includes(event.key)) { event.preventDefault(); setTab((["codex", "workbuddy", "trae"] as TabKey[])[Number(event.key) - 1]); }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh, settingsOpen]);

  useEffect(() => {
    if (!settingsOpen && !menu) return;
    const previous = document.activeElement as HTMLElement | null;
    const surface = settingsOpen ? dialogRef.current : menuRef.current;
    const focusable = () => Array.from(surface?.querySelectorAll<HTMLElement>('button:not(:disabled), textarea, input') || []);
    focusable()[0]?.focus();
    const trap = (event: KeyboardEvent) => {
      const items = focusable();
      if (event.key === "Tab" && items.length) {
        const index = items.indexOf(document.activeElement as HTMLElement);
        event.preventDefault();
        items[(index + (event.shiftKey ? -1 : 1) + items.length) % items.length].focus();
      }
      if (menu && ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
        event.preventDefault();
        const index = items.indexOf(document.activeElement as HTMLElement);
        const next = event.key === "Home" ? 0 : event.key === "End" ? items.length - 1 : (index + (event.key === "ArrowUp" ? -1 : 1) + items.length) % items.length;
        items[next]?.focus();
      }
    };
    window.addEventListener("keydown", trap);
    return () => { window.removeEventListener("keydown", trap); previous?.focus(); };
  }, [settingsOpen, menu]);

  const saveCookie = async () => {
    setSaveError("");
    try {
      await invoke("save_workbuddy_session", { cookie: cookie.trim(), userAgent: userAgent.trim() });
      setSaved(true); setCookie(""); await refresh();
      window.setTimeout(() => setSaved(false), 2200);
    } catch (error) { setSaveError(String(error)); }
  };

  // TRAE：优先保存浏览器会话 Cookie（自动换票、免重贴），否则退回静态 Authorization。
  const saveTrae = async () => {
    setSaveError("");
    try {
      if (traeCookie.trim()) {
        await invoke("save_trae_session", { cookie: traeCookie.trim(), userAgent: traeUserAgent.trim() });
      } else {
        await invoke("save_trae_authorization", { authorization: traeAuthorization.trim() });
      }
      setSaved(true); setTraeCookie(""); setTraeUserAgent(""); setTraeAuthorization(""); await refresh();
      window.setTimeout(() => setSaved(false), 2200);
    } catch (error) { setSaveError(String(error)); }
  };

  // ===== 悬浮球手势：位移 > 6px 视为拖动（移动窗口），否则视为点击（展开面板） =====
  const onBallPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || modeTransition) return;
    ballDrag.current = { pointerId: event.pointerId, startX: event.screenX, startY: event.screenY, dragging: false };
    event.currentTarget.setPointerCapture(event.pointerId);
  };
  const onBallPointerMove = (event: ReactPointerEvent<HTMLDivElement>) => {
    const state = ballDrag.current;
    if (state.pointerId !== event.pointerId || state.dragging) return;
    if (Math.hypot(event.screenX - state.startX, event.screenY - state.startY) > 6) {
      state.dragging = true;
      event.currentTarget.releasePointerCapture(event.pointerId);
      void getCurrentWindow().startDragging().catch(console.error);
    }
  };
  const onBallPointerEnd = (event: ReactPointerEvent<HTMLDivElement>) => {
    const state = ballDrag.current;
    if (state.pointerId !== event.pointerId) return;
    const dragged = state.dragging;
    ballDrag.current = { pointerId: -1, startX: 0, startY: 0, dragging: false };
    if (!dragged) void exitBall("here");
  };

  if (ball && !modeTransition) {
    return (
      <main className="shell ball-shell">
        <div
          className="ball"
          role="button" tabIndex={0} aria-label="展开用量面板"
          onKeyDown={(event) => { if (event.key === "Enter" || event.key === " ") { event.preventDefault(); void exitBall("here"); } }}
          title="点击展开用量面板 · 拖动可移动位置"
          onPointerDown={onBallPointerDown}
          onPointerMove={onBallPointerMove}
          onPointerUp={onBallPointerEnd}
          onPointerCancel={() => { ballDrag.current = { pointerId: -1, startX: 0, startY: 0, dragging: false }; }}
        >
          <BallVisual info={ballInfo(tab, usage)} />
        </div>
      </main>
    );
  }

  const active = usage[tab];

  return (
    <main className={`shell${modeTransition ? ` mode-transition ${modeTransition.direction}${modeTransition.running ? " is-running" : ""}` : ""}`} aria-busy={!!modeTransition} onContextMenu={(event) => {
      if (settingsOpen || modeTransition || /INPUT|TEXTAREA/.test((event.target as HTMLElement).tagName)) return;
      event.preventDefault(); setMenu({ x: Math.max(8, Math.min(event.clientX, window.innerWidth - 216)), y: Math.max(8, Math.min(event.clientY, window.innerHeight - 184)) });
    }}>
      {modeTransition && <div className="morph-surface" aria-hidden="true" />}
      <div className="glass" {...(modeTransition || settingsOpen || menu ? { inert: "" } : {})}>
        <AppToolbar loading={loading} onRefresh={() => void refresh()} onBall={() => void enterBall("right")} onSettings={() => setSettingsOpen(true)} onHide={() => void getCurrentWindow().hide()} />

        <ProviderTabs active={tab} usage={usage} onChange={setTab} />

        <div className="panel-body" id="usage-panel" role="tabpanel" aria-labelledby={`tab-${tab}`}>
          <div className="panel-head">
            <div className="updated">{active.updatedAt ? `${shortTime(active.updatedAt)} 更新` : "等待首次同步"}</div>
            <span className={`plan status-${active.status}`} title={active.error || undefined}>{active.status === "connected" ? "已连接" : active.status === "error" ? "同步异常" : "未连接"}</span>
          </div>
          {active.status === "error" && active.error && <div className="sync-error" role="status"><strong>同步失败详情</strong><span>{active.error}</span></div>}
          {tab === "codex"
            ? <CodexUsage data={usage.codex} onOpenSettings={() => setSettingsOpen(true)} />
            : tab === "workbuddy"
              ? <WorkBuddyUsage data={usage.workbuddy} onOpenSettings={() => setSettingsOpen(true)} />
              : <TraeUsage data={usage.trae} onOpenSettings={() => setSettingsOpen(true)} />}
        </div>

      </div>

      {modeTransition && <div className="morph-ball" aria-hidden="true"><BallVisual info={ballInfo(tab, usage)} /></div>}

      <SettingsDialog
        open={settingsOpen}
        dialogRef={dialogRef}
        cookie={cookie}
        userAgent={userAgent}
        traeAuthorization={traeAuthorization}
        traeCookie={traeCookie}
        traeUserAgent={traeUserAgent}
        saved={saved}
        saveError={saveError}
        preferences={preferences}
        setCookie={setCookie}
        setUserAgent={setUserAgent}
        setTraeAuthorization={setTraeAuthorization}
        setTraeCookie={setTraeCookie}
        setTraeUserAgent={setTraeUserAgent}
        setPreferences={setPreferences}
        onClose={() => setSettingsOpen(false)}
        onSaveWorkbuddy={() => void saveCookie()}
        onSaveTrae={() => void saveTrae()}
      />
      <ContextMenu
        position={menu}
        menuRef={menuRef}
        onDismiss={() => setMenu(null)}
        onRefresh={() => void refresh()}
        onSwitch={() => setTab((["codex", "workbuddy", "trae"] as TabKey[])[((["codex", "workbuddy", "trae"] as TabKey[]).indexOf(tab) + 1) % 3])}
        onSettings={() => setSettingsOpen(true)}
        onBall={() => void enterBall("right")}
      />
    </main>
  );
}

import { useCallback, useEffect, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { Activity, Check, ChevronDown, Pin, RefreshCw, Settings2, X } from "lucide-react";

type UsageWindow = { label: string; usedPercent: number; resetsAt?: number | null };
type ProviderUsage = {
  provider: "codex" | "workbuddy";
  status: "connected" | "needs_login" | "error";
  account?: string | null;
  windows: UsageWindow[];
  credits?: { remaining: number; total: number; unit: string; plan?: string | null } | null;
  updatedAt?: number | null;
  error?: string | null;
};
type UsageSnapshot = { codex: ProviderUsage; workbuddy: ProviderUsage; refreshedAt?: number | null };
type TabKey = "codex" | "workbuddy";
type BallSide = "left" | "right" | "top";

const emptySnapshot: UsageSnapshot = {
  codex: { provider: "codex", status: "needs_login", windows: [] },
  workbuddy: { provider: "workbuddy", status: "needs_login", windows: [] },
};

const tabs: { key: TabKey; name: string; mark: string }[] = [
  { key: "codex", name: "Codex", mark: "C" },
  { key: "workbuddy", name: "WorkBuddy", mark: "W" },
];

/** 拖动停稳后窗口距屏幕边缘小于该物理像素值时吸附成悬浮球。 */
const SNAP_THRESHOLD = 36;
/** 模式切换自身引发的窗口移动在该时间窗内不参与吸附判断，避免抖动循环。 */
const MOVE_SUPPRESS_MS = 1200;
/** onMoved 事件停稳判定时长。 */
const MOVE_SETTLE_MS = 300;

/** Let WebView2 commit a transparent frame before the native window is resized. */
function waitForPaint() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()));
  });
}

const clampPercent = (value: number) => Math.max(0, Math.min(100, value));

function formatReset(seconds?: number | null) {
  if (!seconds) return "等待重置时间";
  const delta = seconds - Date.now() / 1000;
  if (delta <= 0) return "即将重置";
  const hours = Math.floor(delta / 3600);
  const minutes = Math.floor((delta % 3600) / 60);
  return hours ? `${hours} 小时 ${minutes} 分后重置` : `${Math.max(1, minutes)} 分后重置`;
}

function shortTime(seconds?: number | null) {
  if (!seconds) return "尚未刷新";
  return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(new Date(seconds * 1000));
}

/** 悬浮球上展示的当前 Tab 剩余量。 */
function ballInfo(tab: TabKey, usage: UsageSnapshot): { remaining: number | null; mark: string; tone: string } {
  if (tab === "codex") {
    const primary = usage.codex.windows.find((item) => item.label === "5 小时") ?? usage.codex.windows[0];
    const remaining = primary ? 100 - clampPercent(primary.usedPercent) : null;
    return { remaining, mark: "CODEX", tone: toneOf(remaining) };
  }
  const credits = usage.workbuddy.credits;
  const remaining = credits && credits.total > 0 ? Math.round((credits.remaining / credits.total) * 100) : null;
  return { remaining, mark: "WB", tone: toneOf(remaining) };
}

function toneOf(remaining: number | null): string {
  if (remaining === null) return "unknown";
  if (remaining <= 10) return "critical";
  if (remaining <= 30) return "low";
  return "ok";
}

/** 实际悬浮球和转场中的球使用相同的内容，避免最后一帧换形。 */
function BallVisual({ info }: { info: ReturnType<typeof ballInfo> }) {
  const ringLength = 2 * Math.PI * 26;
  const percent = info.remaining === null ? 0 : clampPercent(info.remaining);
  return (
    <div className={`ball-circle ${info.tone}`}>
      <svg className="ball-ring" viewBox="0 0 60 60" aria-hidden="true">
        <circle className="track" cx="30" cy="30" r="26" />
        <circle className="value" cx="30" cy="30" r="26" transform="rotate(-90 30 30)"
          strokeDasharray={ringLength} strokeDashoffset={ringLength * (1 - percent / 100)} />
      </svg>
      <div className="ball-core">
        {info.remaining === null ? <strong className="ball-empty">--</strong> : <strong>{info.remaining}<i>%</i></strong>}
        <span>{info.mark}</span>
      </div>
    </div>
  );
}

function Meter({ percent }: { percent: number }) {
  const safe = clampPercent(percent);
  return <div className="meter"><div style={{ width: `${safe}%` }} /></div>;
}

function EmptyState({ text }: { text: string }) {
  return <div className="empty">{text}</div>;
}

function CodexContent({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  if (!data.windows.length) {
    return (
      <div className="empty-wrap">
        <EmptyState text={data.error || "登录 Codex CLI 后会自动读取用量"} />
        <button className="link-button" onClick={onOpenSettings}>连接帮助</button>
      </div>
    );
  }
  return (
    <>
      {data.account && <div className="account-line">{data.account}</div>}
      {data.windows.map((item) => {
        const remaining = 100 - clampPercent(item.usedPercent);
        return (
          <section className="usage-section" key={item.label}>
            <h2>{item.label}</h2>
            <Meter percent={remaining} />
            <div className="meter-row"><span>剩余 {remaining}%</span><span className="muted">{formatReset(item.resetsAt)}</span></div>
          </section>
        );
      })}
    </>
  );
}

function WorkBuddyContent({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  const remaining = data.credits?.remaining ?? 0;
  const total = data.credits?.total ?? 0;
  const remainingPercent = total > 0 ? Math.round((remaining / total) * 100) : 0;
  const settingsButton = <button className="link-button" onClick={onOpenSettings}>配置会话 <ChevronDown size={13} /></button>;
  if (!data.credits) {
    return (
      <div className="empty-wrap">
        <EmptyState text={data.error || "添加 WorkBuddy 浏览器会话以读取积分"} />
        {settingsButton}
      </div>
    );
  }
  return (
    <>
      <section className="usage-section no-line">
        <div className="credits-total"><strong>{remaining.toLocaleString("zh-CN")}</strong><span>/ {total.toLocaleString("zh-CN")} 积分</span></div>
        <Meter percent={remainingPercent} />
        <div className="meter-row"><span>剩余 {remainingPercent}%</span><span className="muted">{data.credits.plan || "月度积分"}</span></div>
      </section>
      <div className="credits-actions">{settingsButton}</div>
    </>
  );
}

export default function App() {
  const [usage, setUsage] = useState<UsageSnapshot>(emptySnapshot);
  const [loading, setLoading] = useState(false);
  const [tab, setTab] = useState<TabKey>("codex");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [cookie, setCookie] = useState("");
  const [userAgent, setUserAgent] = useState("");
  const [saved, setSaved] = useState(false);
  const [saveError, setSaveError] = useState("");
  const [ball, setBall] = useState<BallSide | null>(null);
  const [modeTransition, setModeTransition] = useState<{ direction: "to-ball" | "to-panel"; running: boolean } | null>(null);

  const ballRef = useRef<BallSide | null>(null);
  const modeChanging = useRef(false);
  const suppressUntil = useRef(0);
  const ballDrag = useRef({ pointerId: -1, startX: 0, startY: 0, dragging: false });

  const refresh = useCallback(async () => {
    setLoading(true);
    try { setUsage(await invoke<UsageSnapshot>("refresh_usage")); }
    catch (error) { console.error(error); }
    finally { setLoading(false); }
  }, []);

  const enterBall = useCallback(async (side: BallSide) => {
    if (ballRef.current || modeChanging.current) return;
    modeChanging.current = true;
    setSettingsOpen(false);
    setModeTransition({ direction: "to-ball", running: false });
    try {
      // 先提交完整的起始帧，再同时启动内容过渡和原生窗口形变。
      await waitForPaint();
      setModeTransition({ direction: "to-ball", running: true });
      await invoke("set_ball_mode", { enabled: true, side, reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches });
      ballRef.current = side;
      setBall(side);
    } catch (error) {
      setBall(null);
      console.error("收起悬浮球失败", error);
    } finally {
      setModeTransition(null);
      suppressUntil.current = Date.now() + MOVE_SUPPRESS_MS;
      modeChanging.current = false;
    }
  }, []);

  const exitBall = useCallback(async (placement: "saved" | "here") => {
    if (!ballRef.current || modeChanging.current) return;
    modeChanging.current = true;
    const side = ballRef.current;
    setModeTransition({ direction: "to-panel", running: false });
    try {
      await waitForPaint();
      setModeTransition({ direction: "to-panel", running: true });
      await invoke("set_ball_mode", { enabled: false, placement, side, reducedMotion: window.matchMedia("(prefers-reduced-motion: reduce)").matches });
      ballRef.current = null;
      setBall(null);
    } catch (error) {
      console.error("展开用量面板失败", error);
    } finally {
      setModeTransition(null);
      suppressUntil.current = Date.now() + MOVE_SUPPRESS_MS;
      modeChanging.current = false;
    }
  }, []);

  // 仅面板拖到边缘时吸附；悬浮球拖动只移动位置。
  useEffect(() => {
    const win = getCurrentWindow();
    let settleTimer = 0;
    let alive = true;

    const check = async () => {
      if (!alive || modeChanging.current || ballRef.current || Date.now() < suppressUntil.current) return;
      let position: { x: number; y: number };
      let size: { width: number; height: number };
      let monitor: { position: { x: number; y: number }; size: { width: number; height: number } } | null;
      try {
        [position, size, monitor] = await Promise.all([win.outerPosition(), win.outerSize(), currentMonitor()]);
      } catch (error) {
        console.error(error);
        return;
      }
      if (!alive || !monitor || modeChanging.current || ballRef.current || Date.now() < suppressUntil.current) return;
      const left = position.x - monitor.position.x;
      const right = monitor.position.x + monitor.size.width - (position.x + size.width);
      const top = position.y - monitor.position.y;

      const side: BallSide | null =
        left <= SNAP_THRESHOLD ? "left"
        : right <= SNAP_THRESHOLD ? "right"
        : top <= SNAP_THRESHOLD ? "top"
        : null;
      if (side) enterBall(side);
    };

    const ready = win.onMoved(() => {
      window.clearTimeout(settleTimer);
      settleTimer = window.setTimeout(() => { void check(); }, MOVE_SETTLE_MS);
    });
    // 托盘"打开用量面板"时，若处于悬浮球模式则自动展开
    const restore = listen("restore-panel", () => { if (ballRef.current) exitBall("saved"); });

    return () => {
      alive = false;
      window.clearTimeout(settleTimer);
      void ready.then((stop) => stop());
      void restore.then((stop) => stop());
    };
  }, [enterBall, exitBall]);

  useEffect(() => {
    invoke<UsageSnapshot>("get_usage").then(setUsage).catch(console.error);
    const timer = window.setInterval(() => { void refresh(); }, 5 * 60 * 1000);
    const unlisten = listen<UsageSnapshot>("usage-updated", (event) => setUsage(event.payload));
    const settingsHandler = () => setSettingsOpen(true);
    window.addEventListener("open-workbuddy-settings", settingsHandler);
    return () => { window.clearInterval(timer); void unlisten.then((stop) => stop()); window.removeEventListener("open-workbuddy-settings", settingsHandler); };
  }, [refresh]);

  const saveCookie = async () => {
    setSaveError("");
    try {
      await invoke("save_workbuddy_session", { cookie: cookie.trim(), userAgent: userAgent.trim() });
      setSaved(true); setCookie(""); await refresh();
      window.setTimeout(() => setSaved(false), 2200);
    } catch (error) { setSaveError(String(error)); }
  };

  // ===== 悬浮球手势：位移 > 6px 视为拖动（移动窗口），否则视为点击（展开面板） =====
  const onBallPointerDown = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || modeChanging.current) return;
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

  const active = tab === "codex" ? usage.codex : usage.workbuddy;

  return (
    <main className={`shell${modeTransition ? ` mode-transition ${modeTransition.direction}${modeTransition.running ? " is-running" : ""}` : ""}`} aria-busy={!!modeTransition}>
      {modeTransition && <div className="morph-surface" aria-hidden="true" />}
      <div className="glass" {...(modeTransition ? { inert: "" } : {})}>
        <header className="app-header" data-tauri-drag-region>
          <div className="brand"><div className="brand-icon"><Activity size={14} strokeWidth={2.4} /></div>CodexBar</div>
          <div className="header-actions">
            <button className={`icon-button ${loading ? "is-loading" : ""}`} aria-label="刷新用量" title="刷新用量" onClick={() => void refresh()}><RefreshCw size={15} /></button>
            <button className="icon-button" aria-label="收起为悬浮球" title="收起为边缘悬浮球（也可直接把窗口拖到屏幕边缘）" onClick={() => enterBall("right")}><Pin size={15} /></button>
            <button className="icon-button" aria-label="设置" title="设置" onClick={() => setSettingsOpen(true)}><Settings2 size={16} /></button>
            <button className="icon-button close-button" aria-label="隐藏窗口" title="隐藏窗口（可从托盘图标恢复）" onClick={() => void getCurrentWindow().hide()}><X size={16} /></button>
          </div>
        </header>

        <nav className="tab-bar">
          {tabs.map((item) => (
            <button key={item.key} className={`tab ${tab === item.key ? "active" : ""}`} onClick={() => setTab(item.key)}>
              <span className="tab-mark">{item.mark}</span>
              <span className="tab-name">{item.name}</span>
              <i className={`tab-dot ${usage[item.key].status}`} />
            </button>
          ))}
        </nav>

        <div className="panel-body">
          <div className="panel-head">
            <div>
              <h1>{tab === "codex" ? "Codex" : "WorkBuddy"}</h1>
              <div className="updated">{shortTime(active.updatedAt)} 更新</div>
            </div>
            <span className="plan">{tab === "codex" ? "OpenAI 订阅" : "腾讯积分"}</span>
          </div>
          {tab === "codex"
            ? <CodexContent data={usage.codex} onOpenSettings={() => setSettingsOpen(true)} />
            : <WorkBuddyContent data={usage.workbuddy} onOpenSettings={() => setSettingsOpen(true)} />}
        </div>

        <footer className="panel-footer">
          <span>凭据保存在本机</span>
          <div className="footer-actions">
            <button onClick={() => void refresh()}>{loading ? "同步中" : "立即刷新"}</button>
            <button onClick={() => setSettingsOpen(true)}>连接帮助</button>
          </div>
        </footer>
      </div>

      {modeTransition && <div className="morph-ball" aria-hidden="true"><BallVisual info={ballInfo(tab, usage)} /></div>}

      {settingsOpen && <div className="modal-backdrop" onMouseDown={(event) => { if (event.target === event.currentTarget) setSettingsOpen(false); }}>
        <section className="settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title">
          <header className="settings-header"><h2 id="settings-title">连接 WorkBuddy</h2><button className="icon-button" aria-label="关闭设置" onClick={() => setSettingsOpen(false)}><X size={16} /></button></header>
          <p className="settings-intro">在浏览器登录 workbuddy.cn，然后从开发者工具的网络请求中复制 Cookie 和 User-Agent。会话凭据保存在 Windows 凭据库中。</p>
          <label className="field-label" htmlFor="cookie">Cookie</label>
          <textarea id="cookie" value={cookie} onChange={(event) => setCookie(event.target.value)} placeholder="粘贴请求头中的 Cookie 内容" rows={4} spellCheck={false} />
          <label className="field-label" htmlFor="ua">User-Agent</label>
          <textarea id="ua" value={userAgent} onChange={(event) => setUserAgent(event.target.value)} placeholder="粘贴同一请求中的 User-Agent" rows={2} spellCheck={false} />
          <div className="privacy-note"><span className="privacy-lock">⌑</span><span>WorkBuddy 会校验 User-Agent。请使用同一浏览器请求里的 Cookie 和 User-Agent。</span></div>
          {saveError && <div className="form-error">{saveError}</div>}
          <div className="settings-actions"><button className="secondary-button" onClick={() => setSettingsOpen(false)}>取消</button><button className="primary-button" disabled={!cookie.trim() || !userAgent.trim()} onClick={() => void saveCookie()}>{saved ? <><Check size={15} />已保存</> : "保存并验证"}</button></div>
        </section>
      </div>}
    </main>
  );
}

import { isTauri } from "@tauri-apps/api/core";
import { Activity, Pin, RefreshCw, Settings2, X } from "lucide-react";

type Props = { loading: boolean; onRefresh: () => void; onBall: () => void; onSettings: () => void; onHide: () => void };
export function AppToolbar({ loading, onRefresh, onBall, onSettings, onHide }: Props) {
  return <header className="app-header" data-tauri-drag-region>
    <div className="brand"><div className="brand-icon"><Activity size={14} strokeWidth={2.4} /></div>CodexBar</div>
    <div className="header-actions">
      <button className={`icon-button ${loading ? "is-loading" : ""}`} aria-label="刷新用量" title="刷新用量" onClick={onRefresh}><RefreshCw size={15} /></button>
      <button className="icon-button" disabled={!isTauri()} aria-label="收起为悬浮球" title="收起为边缘悬浮球（也可直接把窗口拖到屏幕边缘）" onClick={onBall}><Pin size={15} /></button>
      <button className="icon-button" aria-label="设置" title="设置" onClick={onSettings}><Settings2 size={16} /></button>
      <button className="icon-button close-button" disabled={!isTauri()} aria-label="隐藏窗口" title="隐藏窗口（可从托盘图标恢复）" onClick={onHide}><X size={16} /></button>
    </div>
  </header>;
}

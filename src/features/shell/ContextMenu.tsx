import type { RefObject } from "react";
import { Pin } from "lucide-react";

type Props = { position: { x: number; y: number } | null; menuRef: RefObject<HTMLDivElement>; onDismiss: () => void; onRefresh: () => void; onSwitch: () => void; onSettings: () => void; onBall: () => void };

export function ContextMenu({ position, menuRef, onDismiss, onRefresh, onSwitch, onSettings, onBall }: Props) {
  if (!position) return null;
  return <div className="menu-dismiss" onPointerDown={(event) => { if (event.target === event.currentTarget) onDismiss(); }}>
    <div ref={menuRef} className="context-menu" role="menu" aria-label="面板操作" style={{ left: position.x, top: position.y }}>
      <button role="menuitem" onClick={() => { onDismiss(); onRefresh(); }}>刷新用量 <kbd>Ctrl R</kbd></button>
      <button role="menuitem" onClick={() => { onDismiss(); onSwitch(); }}>切换服务 <kbd>Ctrl 1 - 4</kbd></button>
      <button role="menuitem" onClick={() => { onDismiss(); onSettings(); }}>连接与外观 <kbd>Ctrl ,</kbd></button>
      <button role="menuitem" onClick={() => { onDismiss(); onBall(); }}>收起为悬浮球 <Pin size={14} /></button>
    </div>
  </div>;
}

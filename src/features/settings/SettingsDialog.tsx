import { useEffect, useState } from "react";
import { Check, X } from "lucide-react";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import type { AppearancePreferences } from "../../hooks/useAppearancePreferences";

type Props = {
  open: boolean;
  dialogRef: React.RefObject<HTMLElement>;
  cookie: string;
  userAgent: string;
  traeAuthorization: string;
  traeCookie: string;
  traeUserAgent: string;
  saved: boolean;
  saveError: string;
  preferences: AppearancePreferences;
  setCookie: (value: string) => void;
  setUserAgent: (value: string) => void;
  setTraeAuthorization: (value: string) => void;
  setTraeCookie: (value: string) => void;
  setTraeUserAgent: (value: string) => void;
  setPreferences: React.Dispatch<React.SetStateAction<AppearancePreferences>>;
  onClose: () => void;
  onSaveWorkbuddy: () => void;
  onSaveTrae: () => void;
};

export function SettingsDialog({ open, dialogRef, cookie, userAgent, traeAuthorization, traeCookie, traeUserAgent, saved, saveError, preferences, setCookie, setUserAgent, setTraeAuthorization, setTraeCookie, setTraeUserAgent, setPreferences, onClose, onSaveWorkbuddy, onSaveTrae }: Props) {
  const [autostart, setAutostart] = useState(false);
  useEffect(() => {
    isEnabled().then(setAutostart).catch(() => setAutostart(false));
  }, []);
  const toggleAutostart = async () => {
    const next = !autostart;
    setAutostart(next);
    try {
      await (next ? enable() : disable());
    } catch {
      setAutostart(!next);
    }
  };
  return (
    <div className={`modal-backdrop ${open ? "is-open" : ""}`} aria-hidden={!open} {...(!open ? { inert: "" } : {})} onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section ref={dialogRef} className="settings-panel" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <header className="settings-header"><h2 id="settings-title">连接与外观</h2><button className="icon-button" aria-label="关闭设置" onClick={onClose}><X size={16} /></button></header>
        <p className="settings-intro">Codex 登录 CLI 后自动连接。WorkBuddy：在浏览器登录 workbuddy.cn，然后从开发者工具的网络请求中复制 Cookie 和 User-Agent。会话凭据保存在 Windows 凭据库中。</p>
        <label className="field-label" htmlFor="cookie">Cookie</label>
        <textarea id="cookie" value={cookie} onChange={(event) => setCookie(event.target.value)} placeholder="粘贴请求头中的 Cookie 内容" rows={4} spellCheck={false} />
        <label className="field-label" htmlFor="ua">User-Agent</label>
        <textarea id="ua" value={userAgent} onChange={(event) => setUserAgent(event.target.value)} placeholder="粘贴同一请求中的 User-Agent" rows={2} spellCheck={false} />
        <div className="privacy-note"><span className="privacy-lock">⌑</span><span>WorkBuddy 会校验 User-Agent。请使用同一浏览器请求里的 Cookie 和 User-Agent。</span></div>
        <div className="settings-divider" />
        <p className="settings-section-title">TRAE 个人用量</p>
        <p className="settings-help">推荐用浏览器会话 Cookie：登录 www.trae.cn 后，在开发者工具网络面板中任选一个发往 api.trae.cn 的请求，复制它的 Cookie 与 User-Agent。CodexBar 会用该会话在后台自动换取短期 JWT，无需每天重贴。</p>
        <label className="field-label" htmlFor="trae-cookie">Cookie</label>
        <textarea id="trae-cookie" value={traeCookie} onChange={(event) => setTraeCookie(event.target.value)} placeholder="粘贴请求头中的 Cookie 内容" rows={4} spellCheck={false} autoComplete="off" />
        <label className="field-label" htmlFor="trae-ua">User-Agent（可选）</label>
        <textarea id="trae-ua" value={traeUserAgent} onChange={(event) => setTraeUserAgent(event.target.value)} placeholder="留空则使用内置浏览器 User-Agent" rows={2} spellCheck={false} autoComplete="off" />
        <details className="settings-advanced">
          <summary>备用方式：直接粘贴 Authorization</summary>
          <label className="field-label" htmlFor="trae-auth">Authorization</label>
          <textarea id="trae-auth" value={traeAuthorization} onChange={(event) => setTraeAuthorization(event.target.value)} placeholder="Cloud-IDE-JWT …" rows={2} spellCheck={false} autoComplete="off" />
          <p className="settings-help">该值实测只有 8 小时有效期，到期必须重新粘贴；Cookie 填了就优先走自动续期。</p>
        </details>
        {saveError && <div className="form-error">{saveError}</div>}
        <fieldset className="appearance-options"><legend>启动与显示</legend>
          <label><input type="checkbox" checked={autostart} onChange={toggleAutostart} />开机自启动</label>
          <label><input type="checkbox" checked={!!preferences.motion} onChange={(event) => setPreferences({ ...preferences, motion: event.target.checked })} />减少动态效果</label>
          <label><input type="checkbox" checked={!!preferences.transparency} onChange={(event) => setPreferences({ ...preferences, transparency: event.target.checked })} />减少透明度</label>
        </fieldset>
        <div className="settings-actions"><button className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={!cookie.trim() || !userAgent.trim()} onClick={onSaveWorkbuddy}>{saved ? <><Check size={15} />已保存</> : "保存 WorkBuddy"}</button><button className="primary-button" disabled={!traeCookie.trim() && !traeAuthorization.trim()} onClick={onSaveTrae}>{saved ? <><Check size={15} />已保存</> : "保存 TRAE"}</button></div>
      </section>
    </div>
  );
}

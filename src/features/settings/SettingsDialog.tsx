import { Check, X } from "lucide-react";
import type { AppearancePreferences } from "../../hooks/useAppearancePreferences";

type Props = {
  open: boolean;
  dialogRef: React.RefObject<HTMLElement>;
  cookie: string;
  userAgent: string;
  traeAuthorization: string;
  traeDeviceId: string;
  traeCheckinSession: string;
  saved: boolean;
  saveError: string;
  preferences: AppearancePreferences;
  setCookie: (value: string) => void;
  setUserAgent: (value: string) => void;
  setTraeAuthorization: (value: string) => void;
  setTraeDeviceId: (value: string) => void;
  setTraeCheckinSession: (value: string) => void;
  setPreferences: React.Dispatch<React.SetStateAction<AppearancePreferences>>;
  onClose: () => void;
  onSaveWorkbuddy: () => void;
  onSaveTrae: () => void;
  onSaveTraeCheckin: () => void;
};

export function SettingsDialog({ open, dialogRef, cookie, userAgent, traeAuthorization, traeDeviceId, traeCheckinSession, saved, saveError, preferences, setCookie, setUserAgent, setTraeAuthorization, setTraeDeviceId, setTraeCheckinSession, setPreferences, onClose, onSaveWorkbuddy, onSaveTrae, onSaveTraeCheckin }: Props) {
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
        <p className="settings-help">用量查询使用 Authorization。手动签到还需保存登录请求 Cookie 中的 X-Cloudide-Session 和同一请求的 x-device-id。凭据仅保存在本机，并用于直接查询积分或在点击签到时调用 TRAE 接口。</p>
        <label className="field-label" htmlFor="trae-auth">Authorization</label>
        <textarea id="trae-auth" value={traeAuthorization} onChange={(event) => setTraeAuthorization(event.target.value)} placeholder="Cloud-IDE-JWT …" rows={2} spellCheck={false} autoComplete="off" />
        <label className="field-label" htmlFor="trae-device-id">x-device-id（签到必需）</label>
        <input id="trae-device-id" value={traeDeviceId} onChange={(event) => setTraeDeviceId(event.target.value)} placeholder="请求头中的设备 ID" spellCheck={false} autoComplete="off" />
        <label className="field-label" htmlFor="trae-session">签到会话 Cookie</label>
        <textarea id="trae-session" value={traeCheckinSession} onChange={(event) => setTraeCheckinSession(event.target.value)} placeholder="X-Cloudide-Session 的值" rows={2} spellCheck={false} autoComplete="off" />
        <p className="settings-help">在登录后的 TRAE 请求 Cookie 中复制 X-Cloudide-Session 值（可粘贴单独的值或完整 Cookie）。手动签到时应用会用它换取短期 JWT；会话 Cookie 与 x-device-id 仅保存在本机凭据库。会话失效后需重新登录并更新。</p>
        <div className="settings-inline-action"><button className="secondary-button" disabled={!traeCheckinSession.trim() || !traeDeviceId.trim()} onClick={onSaveTraeCheckin}>{saved ? "签到凭据已保存" : "保存签到凭据"}</button></div>
        <div className="privacy-note"><span className="privacy-lock">⌑</span><span>TRAE 网页接口可能变化；凭据仅本机保存。</span></div>
        {saveError && <div className="form-error">{saveError}</div>}
        <fieldset className="appearance-options"><legend>辅助显示</legend>
          <label><input type="checkbox" checked={!!preferences.motion} onChange={(event) => setPreferences({ ...preferences, motion: event.target.checked })} />减少动态效果</label>
          <label><input type="checkbox" checked={!!preferences.transparency} onChange={(event) => setPreferences({ ...preferences, transparency: event.target.checked })} />减少透明度</label>
        </fieldset>
        <div className="settings-actions"><button className="secondary-button" onClick={onClose}>取消</button><button className="primary-button" disabled={!cookie.trim() || !userAgent.trim()} onClick={onSaveWorkbuddy}>{saved ? <><Check size={15} />已保存</> : "保存 WorkBuddy"}</button><button className="primary-button" disabled={!traeAuthorization.trim()} onClick={onSaveTrae}>{saved ? <><Check size={15} />已保存</> : "保存 TRAE"}</button></div>
      </section>
    </div>
  );
}

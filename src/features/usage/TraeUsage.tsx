import { ExternalLink } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { ProviderUsage } from "../../domain/usage";
import { Meter } from "../../components/usage/UsagePrimitives";
import { clampPercent } from "../../lib/usage-format";

const creditNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });

export function TraeUsage({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  if (data.status !== "connected") {
    return <div className="empty-wrap">
      <div className="empty">{data.error || "连接 TRAE 后显示当前周期积分额度"}</div>
      <button className="link-button" onClick={onOpenSettings}>配置 TRAE 授权</button>
      <button className="link-button" onClick={() => void invoke("open_trae_dashboard")}>打开 TRAE 用量页 <ExternalLink size={13} /></button>
    </div>;
  }
  const total = data.credits?.total ?? 0;
  const remaining = data.credits?.remaining ?? 0;
  const used = Math.max(0, total - remaining);
  const usedPercent = clampPercent(data.windows[0]?.usedPercent ?? (total > 0 ? used / total * 100 : 0));
  const remainingPercent = 100 - usedPercent;
  return <>
    <div className="account-line">{data.account}</div>
    <section className="usage-section no-line" aria-label="TRAE 积分用量">
      <h2>当前周期</h2>
      <div className="credits-total"><strong>{creditNumber.format(remaining)}</strong><span>/ {creditNumber.format(total)} 积分</span></div>
      <Meter percent={remainingPercent} />
      <div className="meter-row"><span>剩余 {Math.round(remainingPercent)}%</span><span className="muted">已用 {creditNumber.format(used)} 积分</span></div>
    </section>
    <button className="link-button trae-dashboard-link" onClick={() => void invoke("open_trae_dashboard")}>查看 TRAE 用量后台 <ExternalLink size={13} /></button>
  </>;
}

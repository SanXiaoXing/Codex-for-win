import { CalendarCheck2, ExternalLink, LoaderCircle } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { ProviderUsage } from "../../domain/usage";
import { EmptyState, Meter } from "../../components/usage/UsagePrimitives";
import { clampPercent } from "../../lib/usage-format";

const creditNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 2 });

export function TraeUsage({ data, checkinLoading, checkinMessage, onCheckIn, onOpenSettings }: { data: ProviderUsage; checkinLoading: boolean; checkinMessage: string; onCheckIn: () => void; onOpenSettings: () => void }) {
  const checkinButton = <button className="primary-button trae-checkin-button" disabled={checkinLoading} onClick={onCheckIn}>
    {checkinLoading ? <LoaderCircle size={14} className="spin" /> : <CalendarCheck2 size={14} />}
    {checkinLoading ? "签到中…" : "立即签到"}
  </button>;
  const feedback = checkinMessage && <div className="checkin-feedback" role="status">{checkinMessage}</div>;
  if (data.status !== "connected") {
    return <div className="empty-wrap">
      <EmptyState text={data.error || "连接 TRAE 后显示当前周期积分额度"} />
      <button className="link-button" onClick={onOpenSettings}>配置 TRAE 授权</button>
      {checkinButton}
      {feedback}
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
    {checkinButton}
    {feedback}
    <button className="link-button trae-dashboard-link" onClick={() => void invoke("open_trae_dashboard")}>查看 TRAE 用量后台 <ExternalLink size={13} /></button>
  </>;
}

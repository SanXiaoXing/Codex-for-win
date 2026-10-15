import { ChevronDown } from "lucide-react";
import type { ProviderUsage } from "../../domain/usage";
import { Meter } from "../../components/usage/UsagePrimitives";

export function WorkBuddyUsage({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  const remaining = data.credits?.remaining ?? 0;
  const total = data.credits?.total ?? 0;
  const remainingPercent = total > 0 ? Math.round((remaining / total) * 100) : 0;
  const settingsButton = <button className="link-button" onClick={onOpenSettings}>配置会话 <ChevronDown size={13} /></button>;
  if (!data.credits) {
    return (
      <div className="empty-wrap">
        <div className="empty">{data.error || "添加 WorkBuddy 浏览器会话以读取积分"}</div>
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



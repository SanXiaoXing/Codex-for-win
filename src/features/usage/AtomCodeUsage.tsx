import { ExternalLink } from "lucide-react";
import { invoke } from "@tauri-apps/api/core";
import type { ProviderUsage } from "../../domain/usage";
import { Meter } from "../../components/usage/UsagePrimitives";
import { clampPercent, formatReset } from "../../lib/usage-format";

const metricNumber = new Intl.NumberFormat("zh-CN", { maximumFractionDigits: 0 });

export function AtomCodeUsage({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  if (data.status !== "connected") {
    return <div className="empty-wrap">
      <div className="empty">{data.error || "安装并登录 AtomCode 后自动读取 CodingPlan 用量"}</div>
      <button className="link-button" onClick={onOpenSettings}>连接帮助</button>
      <button className="link-button" onClick={() => void invoke("open_atomcode_page")}>打开 CodingPlan 页 <ExternalLink size={13} /></button>
    </div>;
  }
  return <>
    {data.account && <div className="account-line">{data.account}</div>}
    {data.windows.map((item) => {
      const remaining = 100 - clampPercent(item.usedPercent);
      // 套餐窗口把到期日与剩余天数写在 label 里，滚动窗口才展示「N 分后重置」。
      const isPlan = item.label.startsWith("套餐");
      const credits = data.credits;
      return <section className="usage-section" key={item.label}>
        <h2>{item.label}</h2>
        <Meter percent={remaining} />
        <div className="meter-row">
          <span>剩余 {remaining}%</span>
          {!isPlan && <span className="muted">{formatReset(item.resetsAt)}</span>}
        </div>
        {!isPlan && credits && credits.total > 0 &&
          <div className="meter-row">
            <span className="muted">窗口已用 {metricNumber.format(Math.max(0, credits.total - credits.remaining))} / {metricNumber.format(credits.total)} tokens</span>
          </div>}
      </section>;
    })}
    {data.metrics && data.metrics.length > 0 &&
      <section className="usage-section" aria-label="AtomCode 用量明细">
        <h2>用量明细</h2>
        <dl className="metric-list">
          {data.metrics.map((metric) => <div className="metric-item" key={metric.label}>
            <dt>{metric.label}</dt><dd>{metricNumber.format(metric.value)}</dd>
          </div>)}
        </dl>
      </section>}
    <button className="link-button" onClick={() => void invoke("open_atomcode_page")}>查看 CodingPlan 页 <ExternalLink size={13} /></button>
  </>;
}

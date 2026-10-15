import type { ProviderUsage } from "../../domain/usage";
import { clampPercent, formatReset } from "../../lib/usage-format";
import { Meter } from "../../components/usage/UsagePrimitives";

export function CodexUsage({ data, onOpenSettings }: { data: ProviderUsage; onOpenSettings: () => void }) {
  if (!data.windows.length) {
    return (
      <div className="empty-wrap">
        <div className="empty">{data.error || "登录 Codex CLI 后会自动读取用量"}</div>
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



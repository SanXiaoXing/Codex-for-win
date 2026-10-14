import { providerIcons, tabs } from "../../domain/usage";
import type { TabKey, UsageSnapshot } from "../../domain/usage";

type Props = { active: TabKey; usage: UsageSnapshot; onChange: (tab: TabKey) => void };
export function ProviderTabs({ active, usage, onChange }: Props) {
  return <nav className="tab-bar" role="tablist" aria-label="用量服务">
    <span className={`tab-selection ${active}`} aria-hidden="true" />
    {tabs.map((item) => (
      <button key={item.key} role="tab" id={`tab-${item.key}`} aria-selected={active === item.key} aria-controls="usage-panel" tabIndex={active === item.key ? 0 : -1}
        onKeyDown={(event) => {
          if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
            event.preventDefault();
            const keys: TabKey[] = ["codex", "workbuddy", "trae"];
            const next = event.key === "Home" ? keys[0] : event.key === "End" ? keys[keys.length - 1] : keys[(keys.indexOf(active) + (event.key === "ArrowRight" ? 1 : keys.length - 1)) % keys.length];
            onChange(next);
            document.getElementById(`tab-${next}`)?.focus();
          }
        }}
        className={`tab ${active === item.key ? "active" : ""}`} onClick={() => onChange(item.key)}>
        <img className="tab-mark" src={providerIcons[item.key]} alt="" draggable={false} />
        <span className="tab-name">{item.name}</span>
        <i className={`tab-dot ${usage[item.key].status}`} />
      </button>
    ))}
  </nav>;
}

import { clampPercent } from "../../lib/usage-format";

export function Meter({ percent }: { percent: number }) {
  const safe = clampPercent(percent);
  return <div className="meter" role="meter" aria-label="剩余额度" aria-valuemin={0} aria-valuemax={100} aria-valuenow={safe}><div style={{ transform: `scaleX(${safe / 100})` }} /></div>;
}

export function EmptyState({ text }: { text: string }) {
  return <div className="empty">{text}</div>;
}



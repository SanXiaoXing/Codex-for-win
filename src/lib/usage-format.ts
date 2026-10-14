import type { TabKey, UsageSnapshot, UsageWindow } from "../domain/usage";

export const clampPercent = (value: number) => Math.max(0, Math.min(100, value));

export function formatReset(seconds?: number | null) {
  if (!seconds) return "等待重置时间";
  const delta = seconds - Date.now() / 1000;
  if (delta <= 0) return "即将重置";
  const hours = Math.floor(delta / 3600);
  const minutes = Math.floor((delta % 3600) / 60);
  return hours ? `${hours} 小时 ${minutes} 分后重置` : `${Math.max(1, minutes)} 分后重置`;
}

export function shortTime(seconds?: number | null) {
  if (!seconds) return "尚未刷新";
  return new Intl.DateTimeFormat("zh-CN", { hour: "2-digit", minute: "2-digit" }).format(new Date(seconds * 1000));
}

/** 悬浮球上展示的当前 Tab 剩余量。 */
export function ballInfo(tab: TabKey, usage: UsageSnapshot): { remaining: number | null; mark: string; tone: string } {
  if (tab === "codex") {
    const primary = usage.codex.windows.find((item) => item.label === "5 小时") ?? usage.codex.windows[0];
    const remaining = primary ? 100 - clampPercent(primary.usedPercent) : null;
    return { remaining, mark: "CODEX", tone: toneOf(remaining) };
  }
  if (tab === "trae") {
    const credits = usage.trae.credits;
    const remaining = credits && credits.total > 0
      ? Math.round((credits.remaining / credits.total) * 100)
      : null;
    return { remaining, mark: "TRAE", tone: toneOf(remaining) };
  }
  const credits = usage.workbuddy.credits;
  const remaining = credits && credits.total > 0 ? Math.round((credits.remaining / credits.total) * 100) : null;
  return { remaining, mark: "WB", tone: toneOf(remaining) };
}

function toneOf(remaining: number | null): string {
  if (remaining === null) return "unknown";
  if (remaining <= 10) return "critical";
  if (remaining <= 30) return "low";
  return "ok";
}


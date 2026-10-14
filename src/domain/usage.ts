export type UsageWindow = { label: string; usedPercent: number; resetsAt?: number | null };
export type ProviderUsage = {
  provider: "codex" | "workbuddy" | "trae";
  status: "connected" | "needs_login" | "error";
  account?: string | null;
  windows: UsageWindow[];
  credits?: { remaining: number; total: number; unit: string; plan?: string | null } | null;
  updatedAt?: number | null;
  error?: string | null;
  metrics?: { label: string; value: number; unit: string }[];
};
export type UsageSnapshot = { codex: ProviderUsage; workbuddy: ProviderUsage; trae: ProviderUsage; refreshedAt?: number | null };
export type TabKey = "codex" | "workbuddy" | "trae";
export type BallSide = "left" | "right" | "top";

export const emptySnapshot: UsageSnapshot = {
  codex: { provider: "codex", status: "needs_login", windows: [] },
  workbuddy: { provider: "workbuddy", status: "needs_login", windows: [] },
  trae: { provider: "trae", status: "needs_login", windows: [] },
};

export const tabs: { key: TabKey; name: string }[] = [
  { key: "codex", name: "Codex" },
  { key: "workbuddy", name: "WorkBuddy" },
  { key: "trae", name: "TRAE" },
];

export const providerIcons = {
  codex: new URL("../../src-tauri/icons/codex.svg", import.meta.url).href,
  workbuddy: new URL("../../src-tauri/icons/workbuddy.svg", import.meta.url).href,
  trae: new URL("../../src-tauri/icons/trae.svg", import.meta.url).href,
};

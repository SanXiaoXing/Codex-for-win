export type UsageWindow = { label: string; usedPercent: number; resetsAt?: number | null };
export type UsageMetric = { label: string; value: number };
export type ProviderKey = "codex" | "workbuddy" | "trae" | "atomcode";
export type ProviderUsage = {
  provider: ProviderKey;
  status: "connected" | "needs_login" | "error";
  account?: string | null;
  windows: UsageWindow[];
  credits?: { remaining: number; total: number; plan?: string | null } | null;
  metrics?: UsageMetric[] | null;
  updatedAt?: number | null;
  error?: string | null;
};
export type UsageSnapshot = Record<ProviderKey, ProviderUsage>;
export type TabKey = ProviderKey;
export type BallSide = "left" | "right" | "top";

export const emptySnapshot: UsageSnapshot = {
  codex: { provider: "codex", status: "needs_login", windows: [] },
  workbuddy: { provider: "workbuddy", status: "needs_login", windows: [] },
  trae: { provider: "trae", status: "needs_login", windows: [] },
  atomcode: { provider: "atomcode", status: "needs_login", windows: [] },
};

export const tabs: { key: TabKey; name: string }[] = [
  { key: "codex", name: "Codex" },
  { key: "workbuddy", name: "WorkBuddy" },
  { key: "trae", name: "TRAE" },
  { key: "atomcode", name: "AtomCode" },
];

export const providerIcons = {
  codex: new URL("../../src-tauri/icons/codex.svg", import.meta.url).href,
  workbuddy: new URL("../../src-tauri/icons/workbuddy.svg", import.meta.url).href,
  trae: new URL("../../src-tauri/icons/trae.svg", import.meta.url).href,
  atomcode: new URL("../../src-tauri/icons/atomcode.svg", import.meta.url).href,
};

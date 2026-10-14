import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { emptySnapshot } from "../domain/usage";
import type { UsageSnapshot } from "../domain/usage";

export function useUsageData(onOpenSettings: () => void) {
  const [usage, setUsage] = useState<UsageSnapshot>(emptySnapshot);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    setLoading(true);
    try { setUsage(await invoke<UsageSnapshot>("refresh_usage")); }
    catch (error) { console.error(error); }
    finally { setLoading(false); }
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    invoke<UsageSnapshot>("get_usage").then(setUsage).catch(console.error);
    const timer = window.setInterval(() => { void refresh(); }, 5 * 60 * 1000);
    const unlisten = listen<UsageSnapshot>("usage-updated", (event) => setUsage(event.payload));
    window.addEventListener("open-workbuddy-settings", onOpenSettings);
    return () => {
      window.clearInterval(timer);
      void unlisten.then((stop) => stop());
      window.removeEventListener("open-workbuddy-settings", onOpenSettings);
    };
  }, [onOpenSettings, refresh]);

  return { usage, loading, refresh };
}

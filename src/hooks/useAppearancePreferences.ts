import { useEffect, useState } from "react";

export type AppearancePreferences = { motion?: boolean; transparency?: boolean };

export function useAppearancePreferences() {
  const [preferences, setPreferences] = useState<AppearancePreferences>(() => {
    try { return JSON.parse(localStorage.getItem("appearance") || "{}") as AppearancePreferences; }
    catch { return {}; }
  });

  useEffect(() => {
    document.documentElement.dataset.motion = preferences.motion ? "reduce" : "system";
    document.documentElement.dataset.transparency = preferences.transparency ? "reduce" : "system";
    localStorage.setItem("appearance", JSON.stringify(preferences));
  }, [preferences]);

  return { preferences, setPreferences };
}

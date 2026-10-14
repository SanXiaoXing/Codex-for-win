import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import type { BallSide } from "../domain/usage";

const SNAP_THRESHOLD = 36;
const MOVE_SUPPRESS_MS = 1200;
const MOVE_SETTLE_MS = 300;

function waitForPaint() {
  return new Promise<void>((resolve) => {
    window.requestAnimationFrame(() => window.requestAnimationFrame(() => resolve()));
  });
}

export function useFloatingMode(reduceMotion: boolean) {
  const [ball, setBall] = useState<BallSide | null>(null);
  const [modeTransition, setModeTransition] = useState<{ direction: "to-ball" | "to-panel"; running: boolean } | null>(null);
  const ballRef = useRef<BallSide | null>(null);
  const motionRef = useRef(reduceMotion);
  const modeChanging = useRef(false);
  const suppressUntil = useRef(0);
  motionRef.current = reduceMotion;

  const enterBall = useCallback(async (side: BallSide) => {
    if (!isTauri() || ballRef.current || modeChanging.current) return;
    modeChanging.current = true;
    setModeTransition({ direction: "to-ball", running: false });
    try {
      await waitForPaint();
      setModeTransition({ direction: "to-ball", running: true });
      await invoke("set_ball_mode", { enabled: true, side, reducedMotion: motionRef.current });
      ballRef.current = side;
      setBall(side);
    } catch (error) {
      setBall(null);
      console.error("收起悬浮球失败", error);
    } finally {
      setModeTransition(null);
      suppressUntil.current = Date.now() + MOVE_SUPPRESS_MS;
      modeChanging.current = false;
    }
  }, []);

  const exitBall = useCallback(async (placement: "saved" | "here") => {
    if (!ballRef.current || modeChanging.current) return;
    modeChanging.current = true;
    const side = ballRef.current;
    setModeTransition({ direction: "to-panel", running: false });
    try {
      await waitForPaint();
      setModeTransition({ direction: "to-panel", running: true });
      await invoke("set_ball_mode", { enabled: false, placement, side, reducedMotion: motionRef.current });
      ballRef.current = null;
      setBall(null);
    } catch (error) {
      console.error("展开用量面板失败", error);
    } finally {
      setModeTransition(null);
      suppressUntil.current = Date.now() + MOVE_SUPPRESS_MS;
      modeChanging.current = false;
    }
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    let settleTimer = 0;
    let alive = true;

    const check = async () => {
      if (!alive || modeChanging.current || ballRef.current || Date.now() < suppressUntil.current) return;
      let position: { x: number; y: number };
      let size: { width: number; height: number };
      let monitor: { position: { x: number; y: number }; size: { width: number; height: number } } | null;
      try {
        [position, size, monitor] = await Promise.all([win.outerPosition(), win.outerSize(), currentMonitor()]);
      } catch (error) {
        console.error(error);
        return;
      }
      if (!alive || !monitor || modeChanging.current || ballRef.current || Date.now() < suppressUntil.current) return;
      const left = position.x - monitor.position.x;
      const right = monitor.position.x + monitor.size.width - (position.x + size.width);
      const top = position.y - monitor.position.y;
      const side: BallSide | null = left <= SNAP_THRESHOLD ? "left" : right <= SNAP_THRESHOLD ? "right" : top <= SNAP_THRESHOLD ? "top" : null;
      if (side) void enterBall(side);
    };

    const ready = win.onMoved(() => {
      window.clearTimeout(settleTimer);
      settleTimer = window.setTimeout(() => { void check(); }, MOVE_SETTLE_MS);
    });
    const restore = listen("restore-panel", () => { if (ballRef.current) void exitBall("saved"); });

    return () => {
      alive = false;
      window.clearTimeout(settleTimer);
      void ready.then((stop) => stop());
      void restore.then((stop) => stop());
    };
  }, [enterBall, exitBall]);

  return { ball, modeTransition, enterBall, exitBall };
}

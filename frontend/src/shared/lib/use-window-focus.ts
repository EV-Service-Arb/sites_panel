import { useSyncExternalStore } from "react";

const subscribe = (onChange: () => void) => {
  window.addEventListener("focus", onChange);
  window.addEventListener("blur", onChange);
  document.addEventListener("visibilitychange", onChange);
  return () => {
    window.removeEventListener("focus", onChange);
    window.removeEventListener("blur", onChange);
    document.removeEventListener("visibilitychange", onChange);
  };
};

/** The page is in front of the person: its tab visible and its window focused. */
export function useWindowFocus(): boolean {
  return useSyncExternalStore(
    subscribe,
    () => document.visibilityState === "visible" && document.hasFocus(),
    () => true,
  );
}

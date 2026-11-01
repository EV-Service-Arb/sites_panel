import { awaitElement, containRemoteStyles } from "@evinvest/mfe";
import { useEffect, useState } from "react";

import { WALLET_API_VERSION, WALLET_SCRIPT, WALLET_SCRIPT_INTEGRITY } from "@/shared/config/wallet-bundle";

/** `loading` until the bundle answers; `failed` is final for this mount (the caller keeps its fallback). */
type BundleState = "loading" | "ready" | "failed";

// A bundle that neither loads nor fails (a hung proxy) must not hold the fallback forever.
const TIMEOUT_MS = 8000;

// One load per page: a failed one is forgotten so the next mount retries, a good one is shared. Not
// `@evinvest/mfe`'s loadElement: that one dropped the integrity pin, which banking's contract requires here.
let load: Promise<boolean> | null = null;

function loadOnce(tag: string): Promise<boolean> {
  if (load) return load;
  let failure: Promise<void> = new Promise<void>(() => {});
  if (!customElements.get(tag)) {
    const script = document.createElement("script");
    script.type = "module";
    script.integrity = WALLET_SCRIPT_INTEGRITY;
    script.src = WALLET_SCRIPT;
    failure = new Promise<void>((resolve) => {
      script.addEventListener(
        "error",
        () => {
          script.remove();
          resolve();
        },
        { once: true },
      );
    });
    document.head.appendChild(script);
  }
  const pending = awaitElement(customElements, tag, WALLET_API_VERSION, TIMEOUT_MS, failure);
  load = pending;
  void pending.then((ok) => {
    if (!ok) load = null;
  });
  return pending;
}

/** banking's wallet bundle, pinned by SRI: whether `tag` is usable. A 404, an older bundle and a timeout all read as `failed`. */
export function useWalletBundle(tag: string): BundleState {
  const [state, setState] = useState<BundleState>("loading");
  useEffect(() => {
    let cancelled = false;
    const stopContainment = containRemoteStyles(WALLET_SCRIPT);
    void loadOnce(tag).then((ok) => {
      if (!cancelled) setState(ok ? "ready" : "failed");
    });
    return () => {
      cancelled = true;
      stopContainment();
    };
  }, [tag]);
  return state;
}

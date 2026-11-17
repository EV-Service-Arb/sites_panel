import { describe, expect, it, vi } from "vitest";

import { CallFlow, type CallApi, type CallState, type Visibility } from "@/features/call-lead/model/call-flow";

/** A page whose visibility the test flips, dispatching `visibilitychange` as a browser would. */
function page(): Visibility & { set(hidden: boolean): void } {
  let hidden = false;
  const target = new EventTarget();
  return {
    hidden: () => hidden,
    subscribe: (cb) => {
      target.addEventListener("visibilitychange", cb);
      return () => target.removeEventListener("visibilitychange", cb);
    },
    set(h) {
      hidden = h;
      target.dispatchEvent(new Event("visibilitychange"));
    },
  };
}

function api(): CallApi & { attempt: ReturnType<typeof vi.fn>; outcome: ReturnType<typeof vi.fn> } {
  return { attempt: vi.fn(async () => ({ attemptId: "att-1" })), outcome: vi.fn(async () => null) };
}

const ref = { brand: "aquafix", lead: "L-1" };

describe("calling a lead", () => {
  it("records the attempt as the call starts, then asks for the outcome on return to the tab", async () => {
    const a = api();
    const v = page();
    const states: CallState["phase"][] = [];
    const flow = new CallFlow(a, v, (s) => states.push(s.phase));

    await flow.start(ref);
    expect(a.attempt).toHaveBeenCalledWith(ref);
    expect(flow.current.phase).toBe("dialing");

    v.set(true); // the dialer takes over
    expect(flow.current.phase).toBe("dialing");
    v.set(false); // back in the panel
    expect(flow.current.phase).toBe("asking");

    await flow.answer("no_answer");
    expect(a.outcome).toHaveBeenCalledWith(ref, "att-1", "no_answer");
    expect(flow.current.phase).toBe("idle");
    expect(states).toEqual(["dialing", "asking", "idle"]);
  });

  it("does not ask while the page never left", async () => {
    const v = page();
    const flow = new CallFlow(api(), v, () => {});
    await flow.start(ref);
    v.set(false);
    expect(flow.current.phase).toBe("dialing");
    flow.ask();
    expect(flow.current.phase).toBe("asking");
  });

  it("waits for the attempt when the person is back before the backend answered", async () => {
    let resolve: (answer: { attemptId: string }) => void = () => {};
    const a = api();
    a.attempt.mockImplementation(() => new Promise((r) => (resolve = r)));
    const v = page();
    const flow = new CallFlow(a, v, () => {});
    const started = flow.start(ref);
    v.set(true);
    v.set(false);
    expect(flow.current.phase).toBe("asking");
    const answered = flow.answer("answered");
    resolve({ attemptId: "att-2" });
    await started;
    await answered;
    expect(a.outcome).toHaveBeenCalledWith(ref, "att-2", "answered");
  });

  it("forgets a call whose attempt the backend refused", async () => {
    const a = api();
    const refused = { code: "forbidden", error: "your permissions do not allow this" } as const;
    a.attempt.mockResolvedValue({ failure: refused });
    const v = page();
    const flow = new CallFlow(a, v, () => {});
    expect(await flow.start(ref)).toEqual(refused);
    v.set(true);
    v.set(false);
    expect(flow.current.phase).toBe("idle");
  });

  it("stops listening once disposed", async () => {
    const v = page();
    const flow = new CallFlow(api(), v, () => {});
    await flow.start(ref);
    flow.dispose();
    v.set(true);
    v.set(false);
    expect(flow.current.phase).toBe("dialing");
  });
});

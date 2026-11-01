import { describe, expect, it, vi } from "vitest";

import { ApiError, createHttp } from "@/shared/api/http";

type Call = { url: string; init: RequestInit };

function harness(respond: (call: Call) => Response, cookie = "sa_csrf=tok-dev", csrfCookie = "sa_csrf") {
  const calls: Call[] = [];
  const onUnauthenticated = vi.fn();
  const http = createHttp({
    fetch: async (input, init) => {
      const call = { url: String(input), init: init ?? {} };
      calls.push(call);
      return respond(call);
    },
    cookie: () => cookie,
    host: () => ({ api: "/api/sites", csrfCookie }),
    onUnauthenticated,
  });
  return { http, calls, onUnauthenticated };
}

const json = (status: number, body: unknown) => new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
const headerOf = (c: Call | undefined, name: string) => new Headers(c?.init.headers).get(name);
const any = (v: unknown) => v;

describe("a call", () => {
  it("goes to the host's API: the path joined onto `api`", async () => {
    const { http, calls } = harness(() => json(200, {}));
    await http.get("/leads", any, { stage: "created", brand: null });
    expect(calls.map((c) => c.url)).toEqual(["/api/sites/leads?stage=created"]);
  });
});

describe("the CSRF header", () => {
  it("goes on every write, from the cookie the host names", async () => {
    const { http, calls } = harness(() => json(201, { event_id: "e" }), "sa_csrf=planted; __Host-sa_csrf=tok", "__Host-sa_csrf");
    await http.send("POST", "/leads/aquafix/L-1/stage", { stage: "won" }, any);
    await http.send("DELETE", "/pricing/aquafix", undefined, any);
    expect(calls.map((c) => headerOf(c, "x-sa-csrf"))).toEqual(["tok", "tok"]);
    expect(headerOf(calls[0], "content-type")).toBe("application/json");
  });

  it("is not sent on reads", async () => {
    const { http, calls } = harness(() => json(200, {}));
    await http.get("/me", any);
    expect(headerOf(calls[0], "x-sa-csrf")).toBeNull();
  });

  it("cannot be overridden by a caller's headers", async () => {
    const { http, calls } = harness(() => json(201, {}));
    await http.send("POST", "/leads", { a: 1 }, any, { "x-sa-csrf": "forged", accept: "text/plain" });
    expect(headerOf(calls[0], "x-sa-csrf")).toBe("tok-dev");
    expect(headerOf(calls[0], "accept")).toBe("application/json");
  });
});

describe("the gate's answers", () => {
  it("401 tells the host, which signs the person in", async () => {
    const { http, onUnauthenticated } = harness(() => json(401, { error: "sign in" }));
    await expect(http.get("/me", any)).rejects.toMatchObject({ failure: { kind: "unauthenticated" } });
    expect(onUnauthenticated).toHaveBeenCalledOnce();
  });

  it("403 without a grant is 'no access', a 403 csrf is told apart", async () => {
    const noAccess = harness(() => json(403, { error: "no access to the panel" }));
    await expect(noAccess.http.get("/me", any)).rejects.toMatchObject({ failure: { kind: "no_access" } });
    const csrf = harness(() => json(403, { error: "csrf" }));
    await expect(csrf.http.send("POST", "/leads", {}, any)).rejects.toMatchObject({ failure: { kind: "csrf" } });
  });

  it("503 is 'unavailable' and signs nobody out", async () => {
    const { http, onUnauthenticated } = harness(() => json(503, { error: "sign-in is unavailable, try again" }));
    const err = await http.get("/me", any).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect((err as ApiError).failure.kind).toBe("unavailable");
    expect(onUnauthenticated).not.toHaveBeenCalled();
  });
});

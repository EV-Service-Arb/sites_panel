import { type Api, type Failure as FailureOf, createApi } from "@evinvest/api";

import { host } from "@/shared/config/host";
import { tellHost } from "@/shared/lib/host-events";

import { CSRF_HEADER, readCsrf } from "./csrf";
import { HOP } from "./hop.generated";
import openapi from "./openapi.json";
import type { paths } from "./schema";

/** What the conductor's forward refuses itself, before sites hears the request. */
type Hop = keyof typeof HOP;

export { attemptFor } from "./idempotency";
export type { Attempt } from "./idempotency";

/** Every refusal of the document's and the forward's, and `network`: what a screen shows a failure from. */
export type Failure = FailureOf<paths, Hop>;

let client: Api<paths, Hop> | null = null;

/** The sites API under the host's `api`; `settleHost` refuses a second, other host, so the one made first stays right. */
export function api(): Api<paths, Hop> {
  client ??= createApi<paths, Hop>({
    document: openapi,
    hop: HOP,
    baseUrl: host().api,
    headers: (method) => {
      if (method === "GET") return {};
      const csrf = readCsrf(document.cookie, host().csrfCookie);
      return csrf === null ? {} : { [CSRF_HEADER]: csrf };
    },
    onUnauthenticated: () => tellHost("sa:unauthenticated"),
  });
  return client;
}

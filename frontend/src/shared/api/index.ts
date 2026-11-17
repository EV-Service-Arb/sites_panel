import { type Api, type Failure as FailureOf, createApi } from "@evinvest/api";

import { host } from "@/shared/config/host";
import { tellHost } from "@/shared/lib/host-events";

import { CSRF_HEADER, readCsrf } from "./csrf";
import openapi from "./openapi.json";
import type { paths } from "./schema";

export { attemptFor } from "./idempotency";
export type { Attempt } from "./idempotency";

/** Every refusal of the document's, and `network`: what a screen shows a failure from. */
export type Failure = FailureOf<paths>;

let client: Api<paths> | null = null;

/** The sites API under the host's `api`; `settleHost` refuses a second, other host, so the one made first stays right. */
export function api(): Api<paths> {
  client ??= createApi<paths>({
    document: openapi,
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

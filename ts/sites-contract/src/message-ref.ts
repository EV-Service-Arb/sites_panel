import { LEAD_LIMITS } from "./generated/lead-limits.js";

const MESSAGE_REF = /* @__PURE__ */ new RegExp(LEAD_LIMITS.messageRef);

/** A label pasted with the ref ("Réf. ", "ref ", "Ref: "): the prefilled WhatsApp line ends "Réf. AQ-7K3F". */
const REF_LABEL = /^R[EÉ]F(?:[EÉ]RENCE)?(?:[.:#]\s*|\s+)/;

/** The prefix and the code, apart: a dash, spaces, or both between them. */
const REF_PARTS = /^([A-Z]{2,4})[\s-]*([0-9A-Z]{4,8})$/;

/**
 * A ref as an operator types or pastes it ("aq-7k3f ", "Réf. AQ-7K3F", "ref aq 7k3f", "AQ 7K3F")
 * in the form the landing made it, `AQ-7K3F`, or `null` when it cannot be one
 * (`contracts/vectors/message-ref.json`). In the code Crockford's look-alikes are read as it
 * reads them: O is 0, I and L are 1.
 */
export function messageRefOf(raw: string | null): string | null {
  const text = (raw ?? "").trim().toUpperCase().replace(REF_LABEL, "").trim();
  const parts = REF_PARTS.exec(text);
  const prefix = parts?.[1];
  const code = parts?.[2];
  if (prefix === undefined || code === undefined) return null;
  const ref = `${prefix}-${code.replace(/O/g, "0").replace(/[IL]/g, "1")}`;
  return MESSAGE_REF.test(ref) ? ref : null;
}

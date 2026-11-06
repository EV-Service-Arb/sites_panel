import type { BookingRequested } from "./booking.js";
import type { LeadFlow, LeadSuspect, SiteChannel } from "./generated/lead.js";
import { SCHEMA, type Event, type ExperimentsDeclaredV1, type IngestRequest, type LeadCreatedV1, type Registered, type SiteEventKey, type Subject } from "./generated/wire.js";

/** `lead.created@1`'s properties, their vocabularies as the panel checks them. */
export type LeadCreated = LeadCreatedV1 & { channel: SiteChannel; suspect?: LeadSuspect; flow?: LeadFlow };

/** The properties a site sends for each event it may write, the vocabularies narrowed. */
export type SiteProperties = {
  "lead.created@1": LeadCreated;
  "booking.requested@1": BookingRequested;
  "experiments.declared@1": ExperimentsDeclaredV1;
} & { [K in SiteEventKey]: Registered[K] };

type TypeOf<K extends string> = K extends `${infer T}@${string}` ? T : never;
type VersionOf<K extends string> = K extends `${string}@${infer V extends number}` ? V : never;

/** An event a site sends: `Event` with its type, version and properties known. */
export type SiteEvent<K extends SiteEventKey> = Omit<Event, "type" | "typeVersion" | "properties"> & {
  type: TypeOf<K>;
  typeVersion: VersionOf<K>;
  properties: SiteProperties[K];
};

/** What every event a site sends carries besides its properties. */
export interface SiteEventHead {
  /** A UUIDv7 (`uuidV7`): the panel refuses another version, and keys the journal by it. */
  id: string;
  occurredAt: Date;
  /** The key id the batch is signed with: the panel refuses any other `source.id`. */
  sourceId: string;
  subject: Subject;
}

/**
 * One event as a site sends it (`contracts/vectors/ingest`), and the only way a site builds
 * one: `pii` is what a person typed, kept apart — the panel seals it — and left out when
 * empty.
 */
export function siteEvent<K extends SiteEventKey>(key: K, head: SiteEventHead, properties: SiteProperties[K], pii: Readonly<Record<string, string>> = {}): SiteEvent<K> {
  const [type, version] = key.split("@") as [TypeOf<K>, string];
  return {
    id: head.id,
    schema: SCHEMA,
    type,
    typeVersion: Number(version) as VersionOf<K>,
    occurredAt: head.occurredAt.toISOString(),
    source: { kind: "site", id: head.sourceId },
    subject: head.subject,
    properties,
    ...(Object.keys(pii).length > 0 ? { pii } : {}),
  };
}

/** `POST /api/ingest/v1/events`' body. */
export const ingestBody = (...events: SiteEvent<SiteEventKey>[]): IngestRequest => ({ events });

/** An RFC 9562 UUIDv7 for `atMs`, its other 74 bits random. */
export function uuidV7(atMs: number): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  let ms = atMs;
  for (let i = 5; i >= 0; i--) {
    bytes[i] = ms % 256;
    ms = Math.floor(ms / 256);
  }
  bytes[6] = ((bytes[6] ?? 0) & 0x0f) | 0x70;
  bytes[8] = ((bytes[8] ?? 0) & 0x3f) | 0x80;
  const hex = Array.from(bytes, b => b.toString(16).padStart(2, "0")).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

/**
 * An id the panel takes for a location, a lead or a job: 1–128 of `[A-Za-z0-9._:-]`, starting
 * alphanumeric — and not made only of digits, dots and dashes with seven digits or more, which
 * is what a phone number looks like: an id sits in the clear.
 */
export function isOpaqueId(s: string): boolean {
  if (!/^[A-Za-z0-9][A-Za-z0-9._:-]{0,127}$/.test(s)) return false;
  return !(/^[0-9.-]+$/.test(s) && (s.match(/[0-9]/g)?.length ?? 0) >= 7);
}

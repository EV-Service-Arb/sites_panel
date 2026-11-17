import openapi from "@/shared/api/openapi.json";

const SLUG = new RegExp(openapi.components.schemas.BrandId.pattern);

/** A brand's slug as the document's `BrandId` takes it; a new place's slug, its landing's subdomain, is held to it too. */
export function isSlug(s: string): boolean {
  return SLUG.test(s);
}

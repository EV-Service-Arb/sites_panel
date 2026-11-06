import type { PricingLabels } from "@evinvest/sites-contract";

/** The label for `locale`, or the first one the model has: the panel's locales are not the sites'. */
export function labelOf(labels: PricingLabels, locale: string): string {
  return (Object.hasOwn(labels, locale) ? labels[locale] : undefined) ?? Object.values(labels)[0] ?? "";
}

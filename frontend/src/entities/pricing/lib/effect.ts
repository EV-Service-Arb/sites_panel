import type { PricingInput } from "@evinvest/sites-contract";

/** An option's effect value whatever its kind: cents for `add`, basis points otherwise. */
export function optionValues(input: PricingInput): { id: string; labels: Readonly<Record<string, string>>; value: number }[] {
  switch (input.kind) {
    case "add":
      return input.options.map((o) => ({ id: o.id, labels: o.labels, value: o.addCents }));
    case "multiply":
      return input.options.map((o) => ({ id: o.id, labels: o.labels, value: o.multiplyBp }));
    case "discount":
      return input.options.map((o) => ({ id: o.id, labels: o.labels, value: o.discountBp }));
  }
}

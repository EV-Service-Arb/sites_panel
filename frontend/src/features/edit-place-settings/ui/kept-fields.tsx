import { useT } from "@/shared/i18n";

/** The live fields the panel does not edit in v1: named, so nobody wonders where the address went. */
export function KeptFields({ rest }: { rest: Record<string, unknown> }) {
  const t = useT();
  const keys = Object.keys(rest).sort();
  if (keys.length === 0) return null;
  return <p className="text-sm text-ink-soft">{t("placeSettings.kept", { fields: keys.join(", ") })}</p>;
}

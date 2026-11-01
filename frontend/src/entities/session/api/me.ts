import { type Me, PERMISSIONS, http } from "@/shared/api";
import { arrayOf, object, oneOf, str } from "@/shared/lib/parse";

const meParser = object({ sub: str, permissions: arrayOf(oneOf(PERMISSIONS)) });

export function fetchMe(): Promise<Me> {
  return http.get("/me", meParser);
}

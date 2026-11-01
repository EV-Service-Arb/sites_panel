import { http } from "@/shared/api";

import { type Experiment, type ExperimentPatch, experimentParser, experimentsParser } from "../model/experiment";

/** Every brand's experiments, or one brand's. */
export async function fetchExperiments(brand: string | null): Promise<Experiment[]> {
  return (await http.get("/experiments", experimentsParser, { brand })).experiments;
}

/** An admin's change; the answer is the experiment as it now is. */
export function configureExperiment(brand: string, key: string, patch: ExperimentPatch): Promise<Experiment> {
  return http.send("PUT", `/experiments/${encodeURIComponent(brand)}/${encodeURIComponent(key)}`, patch, experimentParser);
}

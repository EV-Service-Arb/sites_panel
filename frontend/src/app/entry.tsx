import { panelMounted } from "@/shared/lib/router";
import { SitesNav } from "@/views/nav";

import { defineElement } from "./define-element";
import { MeGate } from "./me-gate";
import { PageSkeleton, SitesPanel } from "./sites-panel";

defineElement(
  "mfe-sites-panel",
  () => (
    <MeGate waiting={<PageSkeleton />}>
      <SitesPanel />
    </MeGate>
  ),
  panelMounted,
);

defineElement("mfe-sites-nav", () => (
  <MeGate waiting={null}>
    <SitesNav />
  </MeGate>
));

import { panelMounted } from "@/shared/lib/router";
import { SitesNav } from "@/views/nav";

import { defineElement } from "./define-element";
import { GmbGate } from "./gmb-gate";
import { MeGate } from "./me-gate";
import { PageSkeleton, SitesPanel } from "./sites-panel";

defineElement(
  "mfe-sites-panel",
  () => (
    <MeGate waiting={<PageSkeleton />}>
      <GmbGate waiting={<PageSkeleton />}>
        <SitesPanel />
      </GmbGate>
    </MeGate>
  ),
  panelMounted,
);

defineElement("mfe-sites-nav", () => (
  <MeGate waiting={null}>
    <GmbGate waiting={null}>
      <SitesNav />
    </GmbGate>
  </MeGate>
));

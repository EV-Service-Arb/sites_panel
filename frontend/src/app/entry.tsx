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

defineElement("mfe-sites-nav", (element) => (
  <MeGate waiting={null}>
    <GmbGate waiting={null}>
      <SitesNav element={element} />
    </GmbGate>
  </MeGate>
));

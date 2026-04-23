// Central registry of every Vision view. Each entry maps a stable id -> lazy
// component + presentation metadata for the left nav.

import { lazy, type ComponentType, type LazyExoticComponent } from "react";

export type ViewId =
  | "force-galaxy"
  | "hierarchy-tree"
  | "sunburst"
  | "treemap"
  | "sankey-type"
  | "sankey-domain"
  | "arc-chord"
  | "timeline"
  | "heatmap-grid"
  | "layered-architecture"
  | "project-galaxy-3d"
  | "theme-palette"
  | "test-coverage"
  | "risk-dashboard";

export interface ViewDescriptor {
  id: ViewId;
  label: string;
  shortLabel: string;
  group: "topology" | "flow" | "history" | "quality";
  description: string;
  component: LazyExoticComponent<ComponentType>;
}

export const VIEWS: ViewDescriptor[] = [
  {
    id: "force-galaxy",
    label: "Force Galaxy",
    shortLabel: "Galaxy",
    group: "topology",
    description: "Sigma.js v3 WebGL force-directed view of the whole graph.",
    component: lazy(() => import("./ForceGalaxy").then((m) => ({ default: m.ForceGalaxy }))),
  },
  {
    id: "hierarchy-tree",
    label: "Hierarchy Tree",
    shortLabel: "Tree",
    group: "topology",
    description: "D3 hierarchical tree of folders and files.",
    component: lazy(() => import("./HierarchyTree").then((m) => ({ default: m.HierarchyTree }))),
  },
  {
    id: "sunburst",
    label: "Sunburst",
    shortLabel: "Sunburst",
    group: "topology",
    description: "Radial partition of project mass.",
    component: lazy(() => import("./Sunburst").then((m) => ({ default: m.Sunburst }))),
  },
  {
    id: "treemap",
    label: "Treemap",
    shortLabel: "Treemap",
    group: "topology",
    description: "Squarified treemap weighted by lines or risk.",
    component: lazy(() => import("./Treemap").then((m) => ({ default: m.Treemap }))),
  },
  {
    id: "sankey-type",
    label: "Sankey — Type Flow",
    shortLabel: "Type Flow",
    group: "flow",
    description: "Sankey of type flow between modules.",
    component: lazy(() => import("./SankeyTypeFlow").then((m) => ({ default: m.SankeyTypeFlow }))),
  },
  {
    id: "sankey-domain",
    label: "Sankey — Domain Flow",
    shortLabel: "Domain Flow",
    group: "flow",
    description: "Sankey of domain-to-domain dependencies.",
    component: lazy(() => import("./SankeyDomainFlow").then((m) => ({ default: m.SankeyDomainFlow }))),
  },
  {
    id: "arc-chord",
    label: "Arc Chord",
    shortLabel: "Chord",
    group: "flow",
    description: "Arc/chord diagram of cross-domain calls.",
    component: lazy(() => import("./ArcChord").then((m) => ({ default: m.ArcChord }))),
  },
  {
    id: "timeline",
    label: "Timeline",
    shortLabel: "Timeline",
    group: "history",
    description: "Per-file change timeline; integrates with the time-machine scrubber.",
    component: lazy(() => import("./Timeline").then((m) => ({ default: m.Timeline }))),
  },
  {
    id: "heatmap-grid",
    label: "Heatmap Grid",
    shortLabel: "Heatmap",
    group: "quality",
    description: "Churn vs. complexity heat grid.",
    component: lazy(() => import("./HeatmapGrid").then((m) => ({ default: m.HeatmapGrid }))),
  },
  {
    id: "layered-architecture",
    label: "Layered Architecture",
    shortLabel: "Layers",
    group: "topology",
    description: "Strict-layer view; highlights upward dependencies.",
    component: lazy(() =>
      import("./LayeredArchitecture").then((m) => ({ default: m.LayeredArchitecture })),
    ),
  },
  {
    id: "project-galaxy-3d",
    label: "Project Galaxy 3D",
    shortLabel: "3D Galaxy",
    group: "topology",
    description: "deck.gl + Three.js immersive 3D galaxy.",
    component: lazy(() =>
      import("./ProjectGalaxy3D").then((m) => ({ default: m.ProjectGalaxy3D })),
    ),
  },
  {
    id: "theme-palette",
    label: "Theme Palette",
    shortLabel: "Palette",
    group: "quality",
    description: "CSS variable swatches with WCAG contrast badges.",
    component: lazy(() => import("./ThemePalette").then((m) => ({ default: m.ThemePalette }))),
  },
  {
    id: "test-coverage",
    label: "Test Coverage Map",
    shortLabel: "Coverage",
    group: "quality",
    description: "Per-file coverage heat with gaps highlighted.",
    component: lazy(() =>
      import("./TestCoverageMap").then((m) => ({ default: m.TestCoverageMap })),
    ),
  },
  {
    id: "risk-dashboard",
    label: "Risk Dashboard",
    shortLabel: "Risk",
    group: "quality",
    description: "Risk score breakdown across the project.",
    component: lazy(() => import("./RiskDashboard").then((m) => ({ default: m.RiskDashboard }))),
  },
];

export function getView(id: ViewId): ViewDescriptor {
  const found = VIEWS.find((v) => v.id === id);
  if (!found) throw new Error(`unknown view: ${id}`);
  return found;
}

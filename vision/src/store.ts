import { create } from "zustand";
import { shallow } from "zustand/shallow";
import type { ViewId } from "./views";

export interface NodeRef {
  id: string;
  label?: string;
  type?: string;
}

export interface FilterState {
  type: string[];
  domain: string[];
  search: string;
  riskMin: number;
}

export interface LiveEvent {
  type: string;
  nodeId?: string;
  payload?: unknown;
  ts: number;
}

export interface CommandCenterGoal {
  id: string;
  text: string;
  status: "pending" | "active" | "done" | "blocked";
  parentId?: string;
}

export interface CommandCenterStep {
  id: string;
  description: string;
  status: "todo" | "doing" | "done" | "skipped";
  files?: string[];
  ts: number;
  isCompactionMarker?: boolean;
}

export interface CommandCenterDecision {
  id: string;
  text: string;
  rationale: string;
  ts: number;
}

export interface CommandCenterState {
  goals: CommandCenterGoal[];
  steps: CommandCenterStep[];
  decisions: CommandCenterDecision[];
  constraints: string[];
  filesTouched: string[];
  driftScore: number;
  searchQuery: string;
}

export interface VisionState {
  activeView: ViewId;
  selectedNodes: NodeRef[];
  filters: FilterState;
  timelinePosition: number; // unix ms or commit index
  liveEvents: LiveEvent[];
  projectHash: string;
  commandCenter: CommandCenterState;

  // actions
  setActiveView: (view: ViewId) => void;
  selectNodes: (nodes: NodeRef[]) => void;
  toggleNode: (node: NodeRef) => void;
  clearSelection: () => void;
  setFilter: <K extends keyof FilterState>(key: K, value: FilterState[K]) => void;
  setTimelinePosition: (pos: number) => void;
  pushLiveEvent: (event: LiveEvent) => void;
  setProjectHash: (hash: string) => void;
  upsertGoal: (goal: CommandCenterGoal) => void;
  appendStep: (step: CommandCenterStep) => void;
  appendDecision: (decision: CommandCenterDecision) => void;
  setDriftScore: (score: number) => void;
  setCommandSearch: (query: string) => void;
}

const MAX_LIVE_EVENTS = 500;

const initialFilters: FilterState = {
  type: [],
  domain: [],
  search: "",
  riskMin: 0,
};

const initialCommandCenter: CommandCenterState = {
  goals: [],
  steps: [],
  decisions: [],
  constraints: [],
  filesTouched: [],
  driftScore: 0,
  searchQuery: "",
};

export const useVisionStore = create<VisionState>((set) => ({
  activeView: "force-galaxy",
  selectedNodes: [],
  filters: initialFilters,
  timelinePosition: Date.now(),
  liveEvents: [],
  projectHash: "",
  commandCenter: initialCommandCenter,

  setActiveView: (view) => set({ activeView: view }),

  selectNodes: (nodes) => set({ selectedNodes: nodes }),

  toggleNode: (node) =>
    set((state) => {
      const exists = state.selectedNodes.some((n) => n.id === node.id);
      return {
        selectedNodes: exists
          ? state.selectedNodes.filter((n) => n.id !== node.id)
          : [...state.selectedNodes, node],
      };
    }),

  clearSelection: () => set({ selectedNodes: [] }),

  setFilter: (key, value) =>
    set((state) => ({ filters: { ...state.filters, [key]: value } })),

  setTimelinePosition: (pos) => set({ timelinePosition: pos }),

  pushLiveEvent: (event) =>
    set((state) => {
      const next = [...state.liveEvents, event];
      if (next.length > MAX_LIVE_EVENTS) next.splice(0, next.length - MAX_LIVE_EVENTS);
      return { liveEvents: next };
    }),

  setProjectHash: (hash) => set({ projectHash: hash }),

  upsertGoal: (goal) =>
    set((state) => {
      const idx = state.commandCenter.goals.findIndex((g) => g.id === goal.id);
      const goals = [...state.commandCenter.goals];
      if (idx === -1) goals.push(goal);
      else goals[idx] = goal;
      return { commandCenter: { ...state.commandCenter, goals } };
    }),

  appendStep: (step) =>
    set((state) => ({
      commandCenter: {
        ...state.commandCenter,
        steps: [...state.commandCenter.steps, step],
      },
    })),

  appendDecision: (decision) =>
    set((state) => ({
      commandCenter: {
        ...state.commandCenter,
        decisions: [...state.commandCenter.decisions, decision],
      },
    })),

  setDriftScore: (score) =>
    set((state) => ({
      commandCenter: { ...state.commandCenter, driftScore: score },
    })),

  setCommandSearch: (query) =>
    set((state) => ({
      commandCenter: { ...state.commandCenter, searchQuery: query },
    })),
}));

// Helper exports for convenient selector use.
export const selectFilters = (state: VisionState): FilterState => state.filters;
export const selectActiveView = (state: VisionState): ViewId => state.activeView;
export { shallow };

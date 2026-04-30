// Livebus client — connects to /ws (which is proxied to the supervisor's livebus)
// and dispatches incoming events into the Zustand store. Used by views to pulse
// nodes within ~50ms of a save event.

import { useVisionStore, type LiveEvent } from "./store";

let socket: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectAttempt = 0;
const MAX_BACKOFF = 10_000;

function scheduleReconnect(): void {
  if (reconnectTimer) return;
  const delay = Math.min(MAX_BACKOFF, 500 * 2 ** reconnectAttempt);
  reconnectAttempt += 1;
  reconnectTimer = setTimeout(() => {
    reconnectTimer = null;
    connectLivebus();
  }, delay);
}

export function connectLivebus(): void {
  if (typeof window === "undefined") return;
  if (socket && (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING)) {
    return;
  }
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  const url = `${proto}://${window.location.host}/ws`;
  try {
    socket = new WebSocket(url);
  } catch {
    scheduleReconnect();
    return;
  }

  socket.addEventListener("open", () => {
    reconnectAttempt = 0;
  });

  socket.addEventListener("message", (event) => {
    const raw = typeof event.data === "string" ? event.data : "";
    if (!raw) return;
    try {
      const parsed = JSON.parse(raw) as Partial<LiveEvent> & { type?: string };
      if (!parsed.type) return;
      useVisionStore.getState().pushLiveEvent({
        type: parsed.type,
        nodeId: parsed.nodeId,
        payload: parsed.payload,
        ts: parsed.ts ?? Date.now(),
      });
    } catch {
      // ignore malformed frames; the bus may emit binary heartbeats
    }
  });

  socket.addEventListener("close", () => {
    socket = null;
    scheduleReconnect();
  });

  socket.addEventListener("error", () => {
    try {
      socket?.close();
    } catch {
      /* noop */
    }
  });
}

export function disconnectLivebus(): void {
  if (reconnectTimer) {
    clearTimeout(reconnectTimer);
    reconnectTimer = null;
  }
  try {
    socket?.close();
  } catch {
    /* noop */
  }
  socket = null;
}

export function sendLivebus(payload: unknown): void {
  if (!socket || socket.readyState !== WebSocket.OPEN) return;
  socket.send(typeof payload === "string" ? payload : JSON.stringify(payload));
}

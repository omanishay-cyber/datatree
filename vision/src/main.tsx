import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { connectLivebus } from "./livebus";
import { initProjectSelection } from "./projectSelection";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("vision: #root element missing");

// Read `?project=<hash>` from the URL (or fall back to localStorage)
// BEFORE the first render so the very first fetch picks up the right
// shard instead of hitting the daemon's default-project fallback.
initProjectSelection();

const root = createRoot(container);
root.render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// Open the realtime channel as soon as the renderer mounts.
connectLivebus();

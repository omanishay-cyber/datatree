import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { connectLivebus } from "./livebus";
import "./styles.css";

const container = document.getElementById("root");
if (!container) throw new Error("vision: #root element missing");

const root = createRoot(container);
root.render(
  <StrictMode>
    <App />
  </StrictMode>,
);

// Open the realtime channel as soon as the renderer mounts.
connectLivebus();

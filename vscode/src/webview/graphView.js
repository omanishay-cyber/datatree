// Webview script: handles the two render paths (live iframe vs fallback
// canvas) and bi-directionally chats with the extension host.
//
// We write our own tiny d3-force stand-in instead of pulling the library
// in, since we can't afford a node_modules blow-up for the webview.
// It's a simple Barnes-Hut-free velocity Verlet integrator with three
// forces: charge (repulsion), link (spring), and center gravity.

(function () {
  "use strict";

  const vscode = acquireVsCodeApi();
  const body = document.body;
  const iframe = document.getElementById("mneme-iframe");
  const canvas = document.getElementById("mneme-canvas");
  const tooltip = document.getElementById("mneme-tooltip");
  const statusEl = document.getElementById("mneme-status");
  const refreshBtn = document.getElementById("mneme-refresh");
  const toggleBtn = document.getElementById("mneme-toggle");
  const snapshotNode = document.getElementById("mneme-snapshot");

  const persisted = vscode.getState() || { mode: "iframe", selectedNode: null, zoom: 1 };
  let mode = persisted.mode || "iframe";
  let selectedNode = persisted.selectedNode;

  /** @typedef {{id:string, kind:string, degree:number, file:string|null}} Node */
  /** @typedef {{source:string, target:string}} Edge */

  const snapshot = parseSnapshot(snapshotNode.textContent || "{}");
  /** @type {Node[]} */
  const nodes = snapshot.nodes || [];
  /** @type {Edge[]} */
  const edges = snapshot.edges || [];

  setMode(mode);
  detectIframeHealth();
  setupToolbar();

  if (mode === "fallback" || nodes.length > 0) {
    renderFallback();
  }

  window.addEventListener("message", (event) => {
    const msg = event.data || {};
    if (msg.type === "focusNode" && typeof msg.name === "string") {
      selectedNode = msg.name;
      persist();
      if (mode === "fallback") {
        centerOnNode(msg.name);
      }
    }
  });

  refreshBtn.addEventListener("click", () => {
    vscode.postMessage({ type: "requestRefresh" });
  });

  toggleBtn.addEventListener("click", () => {
    setMode(mode === "iframe" ? "fallback" : "iframe");
    if (mode === "fallback") {
      renderFallback();
    }
  });

  function setMode(next) {
    mode = next;
    body.setAttribute("data-mode", next);
    statusEl.textContent = next === "iframe" ? "Live (vision)" : "Fallback (d3)";
    persist();
  }

  function persist() {
    vscode.setState({ mode, selectedNode, zoom: 1 });
    vscode.postMessage({
      type: "persistState",
      state: { mode, selectedNode, zoom: 1 },
    });
  }

  function detectIframeHealth() {
    if (mode !== "iframe") {
      return;
    }
    let loaded = false;
    iframe.addEventListener("load", () => {
      loaded = true;
      statusEl.textContent = "Live (vision)";
    });
    setTimeout(() => {
      if (!loaded && mode === "iframe") {
        setMode("fallback");
        renderFallback();
      }
    }, 2500);
  }

  function setupToolbar() {
    // Keyboard: "r" = refresh, "t" = toggle.
    document.addEventListener("keydown", (e) => {
      if (e.target && e.target !== document.body) {
        return;
      }
      if (e.key === "r") {
        refreshBtn.click();
      } else if (e.key === "t") {
        toggleBtn.click();
      }
    });
  }

  function parseSnapshot(raw) {
    try {
      return JSON.parse(raw);
    } catch {
      return { nodes: [], edges: [] };
    }
  }

  // ----- Fallback renderer -----

  function renderFallback() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = (canvas.width = canvas.clientWidth || 1200);
    const height = (canvas.height = canvas.clientHeight || 800);

    if (nodes.length === 0) {
      ctx.fillStyle = getCssVar("--mneme-muted", "#888");
      ctx.font = "14px system-ui, sans-serif";
      ctx.textAlign = "center";
      ctx.fillText(
        "No god nodes yet. Run: mneme build .",
        width / 2,
        height / 2,
      );
      return;
    }

    const sim = nodes.map((n, i) => ({
      node: n,
      x: width / 2 + Math.cos((i / nodes.length) * Math.PI * 2) * 200,
      y: height / 2 + Math.sin((i / nodes.length) * Math.PI * 2) * 200,
      vx: 0,
      vy: 0,
      radius: 6 + Math.min(24, Math.sqrt(n.degree || 1) * 3),
    }));
    const idx = new Map(sim.map((p) => [p.node.id, p]));

    const links = edges
      .map((e) => ({ a: idx.get(e.source), b: idx.get(e.target) }))
      .filter((l) => l.a && l.b);

    let frame = 0;
    const maxFrames = 240;

    function tick() {
      // Repulsion.
      for (let i = 0; i < sim.length; i++) {
        for (let j = i + 1; j < sim.length; j++) {
          const a = sim[i];
          const b = sim[j];
          const dx = a.x - b.x;
          const dy = a.y - b.y;
          const distSq = Math.max(25, dx * dx + dy * dy);
          const force = 1400 / distSq;
          const fx = (dx / Math.sqrt(distSq)) * force;
          const fy = (dy / Math.sqrt(distSq)) * force;
          a.vx += fx;
          a.vy += fy;
          b.vx -= fx;
          b.vy -= fy;
        }
      }
      // Springs.
      for (const l of links) {
        const dx = l.b.x - l.a.x;
        const dy = l.b.y - l.a.y;
        const dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const k = 0.02;
        const target = 120;
        const f = (dist - target) * k;
        const fx = (dx / dist) * f;
        const fy = (dy / dist) * f;
        l.a.vx += fx;
        l.a.vy += fy;
        l.b.vx -= fx;
        l.b.vy -= fy;
      }
      // Gravity to center.
      for (const p of sim) {
        p.vx += (width / 2 - p.x) * 0.001;
        p.vy += (height / 2 - p.y) * 0.001;
        p.vx *= 0.82;
        p.vy *= 0.82;
        p.x += p.vx;
        p.y += p.vy;
      }

      drawFrame(ctx, width, height, sim, links);
      frame++;
      if (frame < maxFrames) {
        requestAnimationFrame(tick);
      }
    }
    tick();

    canvas.onmousemove = (evt) => {
      const rect = canvas.getBoundingClientRect();
      const mx = evt.clientX - rect.left;
      const my = evt.clientY - rect.top;
      const hit = sim.find((p) => {
        const dx = p.x - mx;
        const dy = p.y - my;
        return dx * dx + dy * dy <= p.radius * p.radius;
      });
      if (hit) {
        tooltip.hidden = false;
        tooltip.style.left = evt.clientX + 12 + "px";
        tooltip.style.top = evt.clientY + 12 + "px";
        tooltip.textContent = `${hit.node.id} (${hit.node.kind}, deg ${hit.node.degree})`;
      } else {
        tooltip.hidden = true;
      }
    };

    canvas.onclick = (evt) => {
      const rect = canvas.getBoundingClientRect();
      const mx = evt.clientX - rect.left;
      const my = evt.clientY - rect.top;
      const hit = sim.find((p) => {
        const dx = p.x - mx;
        const dy = p.y - my;
        return dx * dx + dy * dy <= p.radius * p.radius;
      });
      if (hit && hit.node.file) {
        vscode.postMessage({
          type: "openFile",
          file: hit.node.file,
          line: 1,
        });
      }
    };

    function centerOnNode(name) {
      const p = idx.get(name);
      if (!p) return;
      // Kick it toward center to "focus".
      p.vx += (width / 2 - p.x) * 0.1;
      p.vy += (height / 2 - p.y) * 0.1;
    }
    // Expose locally for the focusNode handler above.
    renderFallback.centerOnNode = centerOnNode;
  }

  function drawFrame(ctx, width, height, sim, links) {
    ctx.clearRect(0, 0, width, height);

    // Edges.
    ctx.strokeStyle = getCssVar("--mneme-border", "rgba(128,128,128,0.3)");
    ctx.lineWidth = 1;
    for (const l of links) {
      ctx.beginPath();
      ctx.moveTo(l.a.x, l.a.y);
      ctx.lineTo(l.b.x, l.b.y);
      ctx.stroke();
    }

    // Nodes.
    for (const p of sim) {
      ctx.beginPath();
      ctx.fillStyle = colorForKind(p.node.kind);
      ctx.arc(p.x, p.y, p.radius, 0, Math.PI * 2);
      ctx.fill();
      if (selectedNode && p.node.id === selectedNode) {
        ctx.lineWidth = 2;
        ctx.strokeStyle = getCssVar("--mneme-accent", "#007acc");
        ctx.stroke();
      }
    }

    // Labels for the top-8 largest nodes (avoid label soup).
    const topK = [...sim].sort((a, b) => b.radius - a.radius).slice(0, 8);
    ctx.fillStyle = getCssVar("--mneme-fg", "#ddd");
    ctx.font = "11px system-ui, sans-serif";
    ctx.textAlign = "center";
    ctx.textBaseline = "top";
    for (const p of topK) {
      ctx.fillText(truncate(p.node.id, 24), p.x, p.y + p.radius + 4);
    }
  }

  function colorForKind(kind) {
    const lower = (kind || "").toLowerCase();
    if (lower.includes("function") || lower.includes("fn")) return "#66ccff";
    if (lower.includes("class") || lower.includes("struct")) return "#c586c0";
    if (lower.includes("trait") || lower.includes("interface")) return "#4ec9b0";
    if (lower.includes("mod")) return "#d7ba7d";
    return "#858585";
  }

  function getCssVar(name, fallback) {
    const value = getComputedStyle(document.documentElement)
      .getPropertyValue(name)
      .trim();
    return value || fallback;
  }

  function truncate(s, n) {
    if (!s) return "";
    return s.length > n ? s.slice(0, n - 1) + "..." : s;
  }

  function centerOnNode(name) {
    if (typeof renderFallback.centerOnNode === "function") {
      renderFallback.centerOnNode(name);
    }
  }
})();

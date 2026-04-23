import { useEffect, useRef } from "react";
import { Deck } from "@deck.gl/core";
import { PointCloudLayer } from "@deck.gl/layers";
import { OrbitView } from "@deck.gl/core";
import { fetchGraph } from "../api";

interface PointDatum {
  position: [number, number, number];
  color: [number, number, number];
  size: number;
  id: string;
}

export function ProjectGalaxy3D(): JSX.Element {
  const containerRef = useRef<HTMLDivElement | null>(null);
  const deckRef = useRef<Deck | null>(null);

  useEffect(() => {
    const ac = new AbortController();
    let cancelled = false;
    fetchGraph("project-galaxy-3d", { signal: ac.signal }).then((payload) => {
      if (cancelled || !containerRef.current) return;
      const points: PointDatum[] = payload.nodes.map((n, i) => {
        const phi = Math.acos(1 - (2 * (i + 0.5)) / payload.nodes.length);
        const theta = Math.PI * (1 + Math.sqrt(5)) * i;
        const r = 200 + (n.size ?? 4) * 8;
        return {
          id: n.id,
          position: [
            r * Math.cos(theta) * Math.sin(phi),
            r * Math.sin(theta) * Math.sin(phi),
            r * Math.cos(phi),
          ],
          color: [120, 180, 255],
          size: 4 + (n.size ?? 0),
        };
      });

      const layer = new PointCloudLayer<PointDatum>({
        id: "galaxy-points",
        data: points,
        getPosition: (d) => d.position,
        getColor: (d) => d.color,
        pointSize: 4,
        sizeUnits: "pixels",
        opacity: 0.85,
      });

      deckRef.current = new Deck({
        parent: containerRef.current,
        views: [new OrbitView({ orbitAxis: "Y", fov: 50 })],
        initialViewState: { target: [0, 0, 0], rotationX: 25, rotationOrbit: 30, zoom: 1.5 },
        controller: true,
        layers: [layer],
      });
    });
    return () => {
      cancelled = true;
      ac.abort();
      deckRef.current?.finalize();
      deckRef.current = null;
    };
  }, []);

  return (
    <div className="vz-view vz-view--3d">
      <div ref={containerRef} className="vz-view-canvas" data-testid="galaxy-3d" />
      <p className="vz-view-hint">drag to orbit · wheel to zoom</p>
    </div>
  );
}

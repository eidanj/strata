// Crosshair geometry. Kept identical to Crosshair.java in the HUD mod so the
// preview here matches the game pixel for pixel. Units are screen pixels,
// relative to the screen centre.

import type { CrosshairConfig } from "./api";

export interface Rect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

export function crosshairRects(c: CrosshairConfig): Rect[] {
  const T = Math.max(1, Math.round(c.thickness));
  const L = Math.max(0, Math.round(c.length));
  const G = Math.max(0, Math.round(c.gap));
  const r = T % 2; // odd thickness centres on a pixel, even on a line
  const a0 = -Math.floor(T / 2);
  const a1 = a0 + T;
  const out: Rect[] = [];

  switch (c.style) {
    case "plus":
    case "t":
      out.push({ x0: -G - L, y0: a0, x1: -G, y1: a1 }); // left
      out.push({ x0: G + r, y0: a0, x1: G + r + L, y1: a1 }); // right
      out.push({ x0: a0, y0: G + r, x1: a1, y1: G + r + L }); // bottom
      if (c.style === "plus") out.push({ x0: a0, y0: -G - L, x1: a1, y1: -G }); // top
      break;
    case "x":
      for (let i = 0; i < L; i++) {
        const p = G + i + r;
        const n = -G - i - T;
        out.push({ x0: p, y0: p, x1: p + T, y1: p + T });
        out.push({ x0: n, y0: p, x1: n + T, y1: p + T });
        out.push({ x0: p, y0: n, x1: p + T, y1: n + T });
        out.push({ x0: n, y0: n, x1: n + T, y1: n + T });
      }
      break;
    case "circle": {
      const R = G + Math.ceil(L / 2);
      const steps = Math.max(24, Math.round(2 * Math.PI * R));
      const seen = new Set<string>();
      for (let s = 0; s < steps; s++) {
        const th = (s / steps) * Math.PI * 2;
        const x = Math.round(Math.cos(th) * R);
        const y = Math.round(Math.sin(th) * R);
        const key = `${x},${y}`;
        if (seen.has(key)) continue;
        seen.add(key);
        out.push({ x0: x + a0, y0: y + a0, x1: x + a1, y1: y + a1 });
      }
      break;
    }
    case "dot":
      break;
  }

  if (c.dot || c.style === "dot") {
    const D = Math.max(1, Math.round(c.dotSize));
    const d0 = -Math.floor(D / 2);
    out.push({ x0: d0, y0: d0, x1: d0 + D, y1: d0 + D });
  }
  return out;
}

/** Attack-cooldown bar under the crosshair, at `progress` 0..1. */
export function cooldownRects(c: CrosshairConfig, progress: number): { back: Rect; fill: Rect } {
  const span = Math.max(12, 2 * (Math.round(c.gap) + Math.round(c.length)));
  const h = Math.max(2, Math.min(3, Math.round(c.thickness)));
  const top = Math.round(c.gap) + Math.round(c.length) + 5;
  const x0 = -Math.floor(span / 2);
  return {
    back: { x0, y0: top, x1: x0 + span, y1: top + h },
    fill: { x0, y0: top, x1: x0 + Math.round(span * progress), y1: top + h },
  };
}

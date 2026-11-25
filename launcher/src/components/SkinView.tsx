import { useEffect, useRef } from "react";

interface Props {
  /** Skin PNG as a data URL; null draws a plain silhouette. */
  src: string | null;
  slim?: boolean;
  /** Screen pixels per skin pixel. */
  scale?: number;
  label?: string;
}

// Front faces in the standard 64x64 skin layout: [sx, sy, w, h] -> drawn at [dx, dy].
// The figure is 16 skin pixels wide and 32 tall.
type Part = { src: [number, number, number, number]; at: [number, number]; mirrorOf?: boolean };

function parts(slim: boolean, legacy: boolean): { base: Part[]; overlay: Part[] } {
  const aw = slim ? 3 : 4;
  const base: Part[] = [
    { src: [8, 8, 8, 8], at: [4, 0] }, // head
    { src: [20, 20, 8, 12], at: [4, 8] }, // body
    { src: [44, 20, aw, 12], at: [4 - aw, 8] }, // right arm (viewer's left)
    legacy ? { src: [44, 20, aw, 12], at: [12, 8], mirrorOf: true } : { src: [36, 52, aw, 12], at: [12, 8] }, // left arm
    { src: [4, 20, 4, 12], at: [4, 20] }, // right leg
    legacy ? { src: [4, 20, 4, 12], at: [8, 20], mirrorOf: true } : { src: [20, 52, 4, 12], at: [8, 20] }, // left leg
  ];
  const overlay: Part[] = [{ src: [40, 8, 8, 8], at: [4, 0] }]; // hat
  if (!legacy) {
    overlay.push(
      { src: [20, 36, 8, 12], at: [4, 8] }, // jacket
      { src: [44, 36, aw, 12], at: [4 - aw, 8] },
      { src: [52, 52, aw, 12], at: [12, 8] },
      { src: [4, 36, 4, 12], at: [4, 20] },
      { src: [4, 52, 4, 12], at: [8, 20] },
    );
  }
  return { base, overlay };
}

/** Flat front view of a Minecraft skin, drawn pixel-perfect on a canvas. */
export function SkinView({ src, slim = false, scale = 9, label }: Props) {
  const ref = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = ref.current;
    const ctx = canvas?.getContext("2d");
    if (!canvas || !ctx) return;
    ctx.imageSmoothingEnabled = false;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    if (!src) {
      drawSilhouette(ctx, scale);
      return;
    }
    const img = new Image();
    img.onload = () => {
      const legacy = img.height === 32;
      const { base, overlay } = parts(slim, legacy);
      for (const p of [...base, ...overlay]) drawPart(ctx, img, p, scale);
    };
    img.src = src;
  }, [src, slim, scale]);

  return (
    <canvas
      ref={ref}
      className="skin"
      width={16 * scale}
      height={32 * scale}
      role="img"
      aria-label={label ?? "Player skin"}
    />
  );
}

function drawPart(ctx: CanvasRenderingContext2D, img: HTMLImageElement, p: Part, s: number) {
  const [sx, sy, w, h] = p.src;
  const [dx, dy] = p.at;
  if (p.mirrorOf) {
    ctx.save();
    ctx.translate((dx + w) * s, dy * s);
    ctx.scale(-1, 1);
    ctx.drawImage(img, sx, sy, w, h, 0, 0, w * s, h * s);
    ctx.restore();
  } else {
    ctx.drawImage(img, sx, sy, w, h, dx * s, dy * s, w * s, h * s);
  }
}

function drawSilhouette(ctx: CanvasRenderingContext2D, s: number) {
  const fill = (x: number, y: number, w: number, h: number, c: string) => {
    ctx.fillStyle = c;
    ctx.fillRect(x * s, y * s, w * s, h * s);
  };
  fill(4, 0, 8, 8, "#4a535e");
  fill(4, 8, 8, 12, "#3e464f");
  fill(0, 8, 4, 12, "#353d46");
  fill(12, 8, 4, 12, "#353d46");
  fill(4, 20, 4, 12, "#323941");
  fill(8, 20, 4, 12, "#2d343b");
}

export function compact(n: number): string {
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(n);
}

/** "#AARRGGBB" (what the mod reads) <-> "#RRGGBB" + alpha 0..1 (what <input type=color> wants). */
export function splitArgb(argb: string): { hex: string; alpha: number } {
  const v = argb.replace("#", "");
  if (v.length === 8) return { hex: `#${v.slice(2)}`, alpha: parseInt(v.slice(0, 2), 16) / 255 };
  return { hex: `#${v.slice(0, 6)}`, alpha: 1 };
}

export function joinArgb(hex: string, alpha: number): string {
  const a = Math.round(Math.min(1, Math.max(0, alpha)) * 255)
    .toString(16)
    .padStart(2, "0");
  return `#${a}${hex.replace("#", "")}`.toUpperCase();
}

export function initials(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((w) => w[0]!.toUpperCase())
    .join("");
}

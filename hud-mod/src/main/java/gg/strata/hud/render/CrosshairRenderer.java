package gg.strata.hud.render;

import java.util.ArrayList;
import java.util.HashSet;
import java.util.List;
import java.util.Set;

import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;

import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;

import gg.strata.hud.StrataHud;
import gg.strata.hud.config.Colors;
import gg.strata.hud.config.HudConfig;

/**
 * Replaces the vanilla crosshair. Drawn in real screen pixels (not GUI-scaled
 * pixels), so a 2px line is 2px at every GUI scale.
 *
 * The geometry matches launcher/src/lib/crosshair.ts exactly; change both
 * together or the launcher preview will lie.
 */
public final class CrosshairRenderer {
	public record Rect(int x0, int y0, int x1, int y1) {
		Rect grow(int by) {
			return new Rect(x0 - by, y0 - by, x1 + by, y1 + by);
		}
	}

	private CrosshairRenderer() {
	}

	/** Registered via HudElementRegistry.replaceElement(CROSSHAIR, ...). */
	public static void extract(GuiGraphicsExtractor g, DeltaTracker delta, HudElement vanilla) {
		HudConfig.Crosshair c = StrataHud.config().crosshair;
		if (!c.enabled) {
			vanilla.extractRenderState(g, delta);
			return;
		}
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null || mc.player.isSpectator() || !Game.firstPerson(mc)) return;

		float scale = Game.guiScale(mc);
		int cx = Math.round(g.guiWidth() * scale / 2f);
		int cy = Math.round(g.guiHeight() * scale / 2f);

		int color = Colors.parse(c.color, 0xFFFFFFFF);
		if (c.colorOnTarget && Game.aimingAtEntity(mc)) color = Colors.parse(c.targetColor, 0xFFFF5A5A);
		float charge = Game.attackCharge(mc.player);

		Draw.push(g);
		Draw.scale(g, 1f / scale);
		draw(g, c, cx, cy, 1, color, c.cooldownIndicator && charge < 1f ? charge : -1f);
		Draw.pop(g);
	}

	/**
	 * Draw the crosshair centred on (cx, cy), each crosshair pixel {@code px}
	 * units wide. {@code cooldown} 0..1 draws the attack bar; negative hides it.
	 * Also used by the in-game crosshair editor for its enlarged preview.
	 */
	public static void draw(GuiGraphicsExtractor g, HudConfig.Crosshair c, int cx, int cy, int px, int color, float cooldown) {
		List<Rect> rects = rects(c);
		Rect[] bar = cooldown >= 0f ? cooldownRects(c, cooldown) : null;
		int outline = Colors.parse(c.outlineColor, 0xC8000000);

		if (c.outline) {
			for (Rect r : rects) fill(g, r.grow(1), cx, cy, px, outline);
			if (bar != null) fill(g, bar[0].grow(1), cx, cy, px, outline);
		}
		if (bar != null) {
			fill(g, bar[0], cx, cy, px, 0x73000000);
			fill(g, bar[1], cx, cy, px, color);
		}
		for (Rect r : rects) fill(g, r, cx, cy, px, color);
	}

	private static void fill(GuiGraphicsExtractor g, Rect r, int cx, int cy, int px, int color) {
		if (r.x1 <= r.x0 || r.y1 <= r.y0) return;
		Draw.fill(g, cx + r.x0 * px, cy + r.y0 * px, cx + r.x1 * px, cy + r.y1 * px, color);
	}

	public static List<Rect> rects(HudConfig.Crosshair c) {
		int t = Math.max(1, c.thickness);
		int l = Math.max(0, c.length);
		int gap = Math.max(0, c.gap);
		int r = t % 2; // odd thickness centres on a pixel, even on a line
		int a0 = -(t / 2);
		int a1 = a0 + t;
		List<Rect> out = new ArrayList<>();

		switch (c.style) {
			case "plus", "t" -> {
				out.add(new Rect(-gap - l, a0, -gap, a1)); // left
				out.add(new Rect(gap + r, a0, gap + r + l, a1)); // right
				out.add(new Rect(a0, gap + r, a1, gap + r + l)); // bottom
				if (c.style.equals("plus")) out.add(new Rect(a0, -gap - l, a1, -gap)); // top
			}
			case "x" -> {
				for (int i = 0; i < l; i++) {
					int p = gap + i + r;
					int n = -gap - i - t;
					out.add(new Rect(p, p, p + t, p + t));
					out.add(new Rect(n, p, n + t, p + t));
					out.add(new Rect(p, n, p + t, n + t));
					out.add(new Rect(n, n, n + t, n + t));
				}
			}
			case "circle" -> {
				int radius = gap + (l + 1) / 2;
				int steps = Math.max(24, (int) Math.round(2 * Math.PI * radius));
				Set<Long> seen = new HashSet<>();
				for (int s = 0; s < steps; s++) {
					double th = s / (double) steps * Math.PI * 2;
					int x = (int) Math.round(Math.cos(th) * radius);
					int y = (int) Math.round(Math.sin(th) * radius);
					if (!seen.add(((long) x << 32) ^ (y & 0xFFFFFFFFL))) continue;
					out.add(new Rect(x + a0, y + a0, x + a1, y + a1));
				}
			}
			default -> {
				// "dot": only the dot below
			}
		}

		if (c.dot || c.style.equals("dot")) {
			int d = Math.max(1, c.dotSize);
			int d0 = -(d / 2);
			out.add(new Rect(d0, d0, d0 + d, d0 + d));
		}
		return out;
	}

	/** [background, fill] for the attack-cooldown bar. */
	public static Rect[] cooldownRects(HudConfig.Crosshair c, float progress) {
		int span = Math.max(12, 2 * (c.gap + c.length));
		int h = Math.max(2, Math.min(3, c.thickness));
		int top = c.gap + c.length + 5;
		int x0 = -(span / 2);
		return new Rect[] {
			new Rect(x0, top, x0 + span, top + h),
			new Rect(x0, top, x0 + Math.round(span * progress), top + h),
		};
	}
}

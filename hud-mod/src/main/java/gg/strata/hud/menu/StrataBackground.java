package gg.strata.hud.menu;

import net.minecraft.client.gui.GuiGraphicsExtractor;

import gg.strata.hud.config.Colors;
import gg.strata.hud.render.Draw;

/**
 * A cross-section of rock layers drifting slowly, drawn from plain filled
 * columns (no textures). Layers sit lower and move less the deeper they are,
 * and lean a little toward the mouse for depth.
 */
public final class StrataBackground {
	/** Colours top to bottom, toned down so menu text stays readable. */
	private static final int[] COLORS = {
		0xFF6A7652, // lichen
		0xFFA88A5B, // sandstone
		0xFF3A424C, // slate
		0xFF8C5B46, // oxide
		0xFF587080, // shale blue
		0xFF2E353D, // schist
		0xFF20252B, // bedrock
	};
	/** Thickness of each layer as a fraction of screen height. */
	private static final float[] THICKNESS = {0.035f, 0.075f, 0.11f, 0.03f, 0.09f, 0.12f, 0.6f};
	private static final int COLUMN = 2;

	private StrataBackground() {
	}

	/**
	 * @param rise 0..1 entrance progress (layers rise into place on open)
	 * @param seconds time for the drift; pass a constant to hold still
	 */
	public static void draw(GuiGraphicsExtractor g, int width, int height, int mouseX, float rise, float seconds) {
		int horizon = Math.round(height * 0.46f + (1f - rise) * height * 0.35f);
		// Sky: dark at the top, a little lighter at the horizon.
		g.fillGradient(0, 0, width, horizon + 40, 0xFF0E1115, 0xFF1F252C);

		float parallax = (mouseX - width / 2f) / Math.max(1, width);
		for (int x = 0; x < width; x += COLUMN) {
			float nx = x / (float) width;
			float y = horizon + (nx - 0.5f) * height * 0.07f; // the whole stack leans slightly
			for (int i = 0; i < COLORS.length; i++) {
				float depth = i / (float) COLORS.length;
				float wave = (float) (Math.sin(x * (0.018 - depth * 0.008) + seconds * (0.25 - depth * 0.15) + i * 1.7) * (3.5 - depth * 2.5)
						+ Math.sin(x * 0.051 + seconds * 0.11 + i) * 1.2);
				float top = y + wave + parallax * (12f - depth * 10f);
				int y0 = Math.round(top);
				if (y0 >= height) break;
				// Each layer runs to the bottom and the next one is drawn over it,
				// so the waves can never leave gaps between layers.
				Draw.fill(g, x, y0, x + COLUMN, height, COLORS[i]);
				// A lighter seam on top of each layer reads as bedding planes.
				Draw.fill(g, x, y0, x + COLUMN, y0 + 1, Colors.lerp(COLORS[i], 0xFFFFFFFF, 0.14f));
				y = top + THICKNESS[i] * height;
			}
		}
	}

	/** The Strata mark: a rounded tile of tilted layers, {@code size} GUI px square. */
	public static void mark(GuiGraphicsExtractor g, int x, int y, int size) {
		int[] bands = {0xFF2A3038, 0xFF5F7A8C, 0xFF3A424C, 0xFFD3AE72, 0xFFA0674F, 0xFF2A3038, 0xFF7F8F64};
		float[] weights = {1.2f, 0.6f, 1.5f, 0.8f, 0.5f, 1.1f, 0.8f};
		float total = 0;
		for (float w : weights) total += w;
		float cy = y;
		for (int i = 0; i < bands.length; i++) {
			int y0 = Math.round(cy);
			cy += weights[i] / total * size;
			Draw.fill(g, x, y0, x + size, Math.round(cy), bands[i]);
		}
		// Knock out the corners for a rounded look.
		int bg = 0xFF161A1F;
		Draw.fill(g, x, y, x + 1, y + 1, bg);
		Draw.fill(g, x + size - 1, y, x + size, y + 1, bg);
		Draw.fill(g, x, y + size - 1, x + 1, y + size, bg);
		Draw.fill(g, x + size - 1, y + size - 1, x + size, y + size, bg);
	}
}

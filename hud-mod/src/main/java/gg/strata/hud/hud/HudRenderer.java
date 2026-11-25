package gg.strata.hud.hud;

import java.util.List;

import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;

import gg.strata.hud.config.HudConfig;
import gg.strata.hud.hud.modules.ArmorModule;
import gg.strata.hud.hud.modules.CoordsModule;
import gg.strata.hud.hud.modules.CpsModule;
import gg.strata.hud.hud.modules.FpsModule;
import gg.strata.hud.hud.modules.HeldItemsModule;
import gg.strata.hud.hud.modules.KeystrokesModule;
import gg.strata.hud.hud.modules.PingModule;
import gg.strata.hud.hud.modules.ToggleSprintModule;
import gg.strata.hud.render.Draw;
import gg.strata.hud.screen.HudEditorScreen;

/** Draws every enabled module. Registered as one HUD element, just below chat. */
public final class HudRenderer {
	/** Gap kept between modules and the screen edge. */
	public static final int MARGIN = 4;

	public static final List<HudModule> MODULES = List.of(
			new FpsModule(),
			new CpsModule(),
			new PingModule(),
			new KeystrokesModule(),
			new ArmorModule(),
			new HeldItemsModule(),
			new ToggleSprintModule(),
			new CoordsModule());

	private HudRenderer() {
	}

	public static void extract(GuiGraphicsExtractor g, DeltaTracker delta) {
		Minecraft mc = Minecraft.getInstance();
		if (mc.player == null) return;
		InputStats.frame(mc);

		boolean editing = HudEditorScreen.isOpen();
		int screenW = g.guiWidth();
		int screenH = g.guiHeight();

		for (HudModule m : MODULES) {
			HudConfig.ModuleSettings s = m.settings();
			m.drawn = false;
			// Hidden modules still draw in the editor (dimmed there) so they can be re-enabled.
			if (!s.enabled && !editing) continue;
			m.layout(mc, editing);
			if (m.width() <= 0) continue;

			float scale = (float) Math.max(0.5, Math.min(2.5, s.scale));
			int w = Math.round(m.width() * scale);
			int h = Math.round(m.height() * scale);
			int x = toPixels(s.x, w, screenW);
			int y = toPixels(s.y, h, screenH);

			Draw.push(g);
			Draw.translate(g, x, y);
			Draw.scale(g, scale);
			m.draw(g, mc, editing);
			Draw.pop(g);

			m.boxX = x;
			m.boxY = y;
			m.boxW = w;
			m.boxH = h;
			m.drawn = true;
		}
	}

	/** Fraction of free space (0..1) to a pixel position, kept on screen. */
	public static int toPixels(double fraction, int size, int screen) {
		int free = screen - size - MARGIN * 2;
		if (free <= 0) return MARGIN;
		return MARGIN + (int) Math.round(Math.max(0, Math.min(1, fraction)) * free);
	}

	public static double toFraction(int pixels, int size, int screen) {
		int free = screen - size - MARGIN * 2;
		if (free <= 0) return 0;
		return Math.max(0, Math.min(1, (pixels - MARGIN) / (double) free));
	}
}

package gg.strata.hud.hud.modules;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;

import gg.strata.hud.config.Colors;
import gg.strata.hud.hud.HudModule;
import gg.strata.hud.hud.InputStats;
import gg.strata.hud.render.Draw;
import gg.strata.hud.render.Game;

/**
 * W A S D, mouse buttons with CPS, and jump. Keys fade in and out over ~70 ms
 * so fast taps are still visible. Labels follow the player's own bindings.
 */
public final class KeystrokesModule extends HudModule {
	private static final int KEY = 22;
	private static final int GAP = 2;
	private static final int WIDE = KEY * 3 + GAP * 2;
	private static final float FADE_NANOS = 70_000_000f;

	// up, left, down, right, attack, use, jump
	private final float[] glow = new float[7];
	private long lastFrame;

	public KeystrokesModule() {
		super("keystrokes", "Keystrokes");
	}

	private boolean showMouse() {
		Boolean b = settings().showMouse;
		return b == null || b;
	}

	private boolean showSpace() {
		Boolean b = settings().showSpace;
		return b == null || b;
	}

	@Override
	public void layout(Minecraft mc, boolean editing) {
		width = WIDE;
		height = KEY * 2 + GAP;
		if (showMouse()) height += GAP + KEY;
		if (showSpace()) height += GAP + 10;
	}

	@Override
	public void draw(GuiGraphicsExtractor g, Minecraft mc, boolean editing) {
		long now = System.nanoTime();
		float step = lastFrame == 0 ? 1f : Math.min(1f, (now - lastFrame) / FADE_NANOS);
		lastFrame = now;

		KeyMapping[] keys = {
			mc.options.keyUp, mc.options.keyLeft, mc.options.keyDown, mc.options.keyRight,
			mc.options.keyAttack, mc.options.keyUse, mc.options.keyJump,
		};
		for (int i = 0; i < keys.length; i++) {
			float target = keys[i].isDown() ? 1f : 0f;
			glow[i] += Math.signum(target - glow[i]) * Math.min(step, Math.abs(target - glow[i]));
		}

		int x1 = KEY + GAP;
		int x2 = (KEY + GAP) * 2;
		key(g, x1, 0, KEY, KEY, Game.keyLabel(keys[0], "W"), null, glow[0]);
		key(g, 0, KEY + GAP, KEY, KEY, Game.keyLabel(keys[1], "A"), null, glow[1]);
		key(g, x1, KEY + GAP, KEY, KEY, Game.keyLabel(keys[2], "S"), null, glow[2]);
		key(g, x2, KEY + GAP, KEY, KEY, Game.keyLabel(keys[3], "D"), null, glow[3]);

		int y = KEY * 2 + GAP * 2;
		if (showMouse()) {
			int half = (WIDE - GAP) / 2;
			key(g, 0, y, half, KEY, "LMB", InputStats.leftCps() + " CPS", glow[4]);
			key(g, half + GAP, y, WIDE - half - GAP, KEY, "RMB", InputStats.rightCps() + " CPS", glow[5]);
			y += KEY + GAP;
		}
		if (showSpace()) {
			int bg = Colors.lerp(Draw.background, Draw.pressed, glow[6]);
			Draw.fill(g, 0, y, WIDE, y + 10, bg);
			int bar = Colors.lerp(Draw.text, Draw.pressedText, glow[6]);
			Draw.fill(g, WIDE / 2 - 12, y + 4, WIDE / 2 + 12, y + 5, bar);
		}
	}

	private static void key(GuiGraphicsExtractor g, int x, int y, int w, int h, String label, String sub, float t) {
		Draw.fill(g, x, y, x + w, y + h, Colors.lerp(Draw.background, Draw.pressed, t));
		int fg = Colors.lerp(Draw.text, Draw.pressedText, t);
		boolean shadow = Draw.shadow && t < 0.5f;
		if (sub == null) {
			Draw.text(g, label, x + (w - Draw.width(label)) / 2, y + (h - 8) / 2, fg, shadow);
		} else {
			Draw.text(g, label, x + (w - Draw.width(label)) / 2, y + 3, fg, shadow);
			// CPS underneath at half size.
			Draw.push(g);
			Draw.translate(g, x + w / 2f, y + 13);
			Draw.scale(g, 0.5f);
			Draw.text(g, sub, -Draw.width(sub) / 2, 0, fg, false);
			Draw.pop(g);
		}
	}
}

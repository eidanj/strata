package gg.strata.hud.menu;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.network.chat.Component;

import gg.strata.hud.config.Colors;
import gg.strata.hud.render.Draw;

/**
 * Flat menu button: label on the left, an accent bar that grows on hover or
 * keyboard focus. {@code link} buttons are plain text that underline instead.
 * In 26.3 a Button subclass draws everything itself in extractContents.
 */
public final class MenuButton extends Button {
	private static final int ACCENT = 0xFF9A86B5;
	private final boolean link;
	private float glow;
	private long lastFrame;

	public MenuButton(int x, int y, int width, int height, Component label, OnPress onPress, boolean link) {
		super(x, y, width, height, label, onPress, DEFAULT_NARRATION);
		this.link = link;
	}

	@Override
	protected void extractContents(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		boolean hot = this.active && this.isHoveredOrFocused();
		long now = System.nanoTime();
		float step = lastFrame == 0 ? 1f : Math.min(1f, (now - lastFrame) / 110_000_000f);
		lastFrame = now;
		float target = hot ? 1f : 0f;
		glow += Math.signum(target - glow) * Math.min(step, Math.abs(target - glow));

		int x = getX();
		int y = getY();
		int w = getWidth();
		int h = getHeight();
		String label = getMessage().getString();
		int textY = y + (h - 8) / 2;

		if (link) {
			int color = Colors.lerp(Draw.muted, Draw.text, glow);
			Draw.text(g, label, x, textY, color, false);
			if (glow > 0.5f) Draw.fill(g, x, textY + 9, x + Draw.width(label), textY + 10, color);
			return;
		}

		Draw.fill(g, x, y, x + w, y + h, Colors.lerp(0x0AFFFFFF, 0x24FFFFFF, glow));
		int bar = 2 + Math.round(glow * 2);
		Draw.fill(g, x, y, x + bar, y + h, Colors.lerp(0x559A86B5, ACCENT, glow));
		int color = this.active ? Colors.lerp(0xFFD5D2CB, 0xFFFFFFFF, glow) : Draw.muted;
		Draw.text(g, label, x + 10 + Math.round(glow * 3), textY, color, Draw.shadow);
	}
}

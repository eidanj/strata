package gg.strata.hud.render;

import org.jspecify.annotations.Nullable;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.item.ItemStack;

import gg.strata.hud.config.Colors;
import gg.strata.hud.config.HudConfig;

/**
 * Every GUI drawing call the mod makes goes through here. Minecraft renames
 * these often (26.1 turned GuiGraphics into GuiGraphicsExtractor and
 * drawString into text), so when a new version breaks the build, this is the
 * file to fix. Names below were checked against Fabric API and JEI for 26.3.
 */
public final class Draw {
	/** Resolved theme colours; refreshed whenever the config changes. */
	public static int text, muted, accent, background, pressed, pressedText;
	public static boolean shadow;

	private Draw() {
	}

	public static void applyTheme(HudConfig.Theme t) {
		text = Colors.parse(t.text, 0xFFE6E3DC);
		muted = Colors.parse(t.muted, 0xFF9AA3AD);
		accent = Colors.parse(t.accent, 0xFFD3AE72);
		background = Colors.parse(t.background, 0x99161A1F);
		pressed = Colors.parse(t.pressed, 0xE6E6E3DC);
		pressedText = Colors.parse(t.pressedText, 0xFF161A1F);
		shadow = t.textShadow;
	}

	public static Font font() {
		return Minecraft.getInstance().font;
	}

	public static int width(String s) {
		return font().width(s);
	}

	public static int lineHeight() {
		return font().lineHeight;
	}

	/** Filled rectangle, corners (x0,y0) inclusive to (x1,y1) exclusive. */
	public static void fill(GuiGraphicsExtractor g, int x0, int y0, int x1, int y1, int argb) {
		g.fill(x0, y0, x1, y1, argb);
	}

	/** One-pixel border inside the given box. */
	public static void outline(GuiGraphicsExtractor g, int x, int y, int w, int h, int argb) {
		g.fill(x, y, x + w, y + 1, argb);
		g.fill(x, y + h - 1, x + w, y + h, argb);
		g.fill(x, y + 1, x + 1, y + h - 1, argb);
		g.fill(x + w - 1, y + 1, x + w, y + h - 1, argb);
	}

	public static void text(GuiGraphicsExtractor g, String s, int x, int y, int argb) {
		g.text(font(), s, x, y, argb, shadow);
	}

	public static void text(GuiGraphicsExtractor g, String s, int x, int y, int argb, boolean withShadow) {
		g.text(font(), s, x, y, argb, withShadow);
	}

	public static void centeredText(GuiGraphicsExtractor g, String s, int centerX, int y, int argb) {
		g.text(font(), s, centerX - width(s) / 2, y, argb, shadow);
	}

	/** 16x16 item icon plus the vanilla count and durability bar. */
	public static void item(GuiGraphicsExtractor g, ItemStack stack, int x, int y, @Nullable String countText) {
		g.item(stack, x, y);
		g.itemDecorations(font(), stack, x, y, countText);
	}

	public static void push(GuiGraphicsExtractor g) {
		g.pose().pushMatrix();
	}

	public static void pop(GuiGraphicsExtractor g) {
		g.pose().popMatrix();
	}

	public static void translate(GuiGraphicsExtractor g, float x, float y) {
		g.pose().translate(x, y);
	}

	public static void scale(GuiGraphicsExtractor g, float s) {
		g.pose().scale(s);
	}
}

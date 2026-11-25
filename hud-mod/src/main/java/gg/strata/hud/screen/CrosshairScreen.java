package gg.strata.hud.screen;

import java.util.List;
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import java.util.function.Supplier;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.network.chat.Component;

import gg.strata.hud.StrataHud;
import gg.strata.hud.config.Colors;
import gg.strata.hud.config.HudConfig;
import gg.strata.hud.render.CrosshairRenderer;
import gg.strata.hud.render.Draw;

/**
 * In-game crosshair editor with a live, enlarged preview. Every change is
 * applied immediately, so the real crosshair at the centre updates too.
 * The launcher's HUD tab edits the same settings with a colour picker.
 */
public final class CrosshairScreen extends Screen {
	private static final List<String> STYLES = List.of("plus", "t", "x", "circle", "dot");
	private static final List<String> STYLE_NAMES = List.of("Plus", "T", "X", "Circle", "Dot");

	private record Swatch(String name, String argb) {
	}

	private static final List<Swatch> SWATCHES = List.of(
			new Swatch("White", "#FFFFFFFF"),
			new Swatch("Green", "#FF55FF55"),
			new Swatch("Cyan", "#FF55FFFF"),
			new Swatch("Yellow", "#FFFFFF55"),
			new Swatch("Magenta", "#FFFF55FF"),
			new Swatch("Red", "#FFFF5A5A"),
			new Swatch("Black", "#FF000000"));

	private final Screen parent;
	private final HudConfig.Crosshair c = StrataHud.config().crosshair;
	private boolean previewTarget;

	public CrosshairScreen(Screen parent) {
		super(Component.literal("Crosshair"));
		this.parent = parent;
	}

	@Override
	protected void init() {
		int x = 16;
		int w = 170;
		int y = 28;
		int step = 23;

		toggle(x, y, w, "Strata crosshair", () -> c.enabled, v -> c.enabled = v);
		y += step;
		addRenderableWidget(Button.builder(styleLabel(), b -> {
			c.style = STYLES.get((STYLES.indexOf(c.style) + 1) % STYLES.size());
			b.setMessage(styleLabel());
		}).bounds(x, y, w, 20).build());
		y += step;
		addRenderableWidget(new IntSlider(x, y, w, "Length", 1, 20, c.length, v -> c.length = v));
		y += step;
		addRenderableWidget(new IntSlider(x, y, w, "Gap", 0, 12, c.gap, v -> c.gap = v));
		y += step;
		addRenderableWidget(new IntSlider(x, y, w, "Thickness", 1, 6, c.thickness, v -> c.thickness = v));
		y += step;
		toggle(x, y, w, "Centre dot", () -> c.dot, v -> c.dot = v);
		y += step;
		addRenderableWidget(new IntSlider(x, y, w, "Dot size", 1, 8, c.dotSize, v -> c.dotSize = v));
		y += step;
		swatch(x, y, w, "Colour", () -> c.color, v -> c.color = v);
		y += step;
		toggle(x, y, w, "Outline", () -> c.outline, v -> c.outline = v);
		y += step;
		toggle(x, y, w, "Colour on target", () -> c.colorOnTarget, v -> c.colorOnTarget = v);
		y += step;
		swatch(x, y, w, "Target colour", () -> c.targetColor, v -> c.targetColor = v);
		y += step;
		toggle(x, y, w, "Cooldown bar", () -> c.cooldownIndicator, v -> c.cooldownIndicator = v);
		y += step + 6;
		addRenderableWidget(Button.builder(Component.literal("Done"), b -> onClose()).bounds(x, y, w, 20).build());

		int px = previewCenterX();
		addRenderableWidget(Button.builder(targetLabel(), b -> {
			previewTarget = !previewTarget;
			b.setMessage(targetLabel());
		}).bounds(px - 80, this.height / 2 + 84, 160, 20).build());
	}

	@Override
	public void onClose() {
		this.minecraft.gui.setScreen(parent);
	}

	@Override
	public void removed() {
		StrataHud.save();
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		Draw.fill(g, 0, 0, 200, this.height, 0xCC161A1F);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		super.extractRenderState(g, mouseX, mouseY, delta);
		Draw.text(g, "Crosshair", 16, 12, Draw.text);

		// Enlarged preview: 5 GUI pixels per crosshair pixel on a slate tile.
		int cx = previewCenterX();
		int cy = this.height / 2;
		Draw.fill(g, cx - 75, cy - 75, cx + 75, cy + 75, 0xE0161A1F);
		Draw.outline(g, cx - 75, cy - 75, 150, 150, 0x33FFFFFF);
		int color = Colors.parse(previewTarget && c.colorOnTarget ? c.targetColor : c.color, 0xFFFFFFFF);
		if (c.enabled) {
			CrosshairRenderer.draw(g, c, cx, cy, 5, color, c.cooldownIndicator ? 0.62f : -1f);
		} else {
			Draw.centeredText(g, "Using the vanilla crosshair", cx, cy - 4, Draw.muted);
		}
		Draw.centeredText(g, "5x preview. The real one is at the centre of the screen.", cx, cy + 80 + 28, Draw.muted);
	}

	private int previewCenterX() {
		return 200 + (this.width - 200) / 2;
	}

	private Component styleLabel() {
		return Component.literal("Shape: " + STYLE_NAMES.get(Math.max(0, STYLES.indexOf(c.style))));
	}

	private Component targetLabel() {
		return Component.literal(previewTarget ? "Preview: aiming at a player" : "Preview: aiming at nothing");
	}

	private void toggle(int x, int y, int w, String label, Supplier<Boolean> get, Consumer<Boolean> set) {
		addRenderableWidget(Button.builder(onOff(label, get.get()), b -> {
			set.accept(!get.get());
			b.setMessage(onOff(label, get.get()));
		}).bounds(x, y, w, 20).build());
	}

	private static Component onOff(String label, boolean on) {
		return Component.literal(label + ": " + (on ? "On" : "Off"));
	}

	private void swatch(int x, int y, int w, String label, Supplier<String> get, Consumer<String> set) {
		addRenderableWidget(Button.builder(swatchLabel(label, get.get()), b -> {
			int i = indexOf(get.get());
			set.accept(SWATCHES.get((i + 1) % SWATCHES.size()).argb());
			b.setMessage(swatchLabel(label, get.get()));
		}).bounds(x, y, w, 20).build());
	}

	private static int indexOf(String argb) {
		for (int i = 0; i < SWATCHES.size(); i++) {
			if (SWATCHES.get(i).argb().equalsIgnoreCase(argb)) return i;
		}
		return -1; // custom colour from the launcher; next click starts the cycle
	}

	private static Component swatchLabel(String label, String argb) {
		int i = indexOf(argb);
		return Component.literal(label + ": " + (i < 0 ? "Custom" : SWATCHES.get(i).name()));
	}

	/** Whole-number slider: "Gap: 3 px". */
	private static final class IntSlider extends AbstractSliderButton {
		private final String label;
		private final int min;
		private final int max;
		private final IntConsumer onChange;

		IntSlider(int x, int y, int w, String label, int min, int max, int value, IntConsumer onChange) {
			super(x, y, w, 20, Component.empty(), (value - min) / (double) (max - min));
			this.label = label;
			this.min = min;
			this.max = max;
			this.onChange = onChange;
			updateMessage();
		}

		private int current() {
			return min + (int) Math.round(this.value * (max - min));
		}

		@Override
		protected void updateMessage() {
			setMessage(Component.literal(label + ": " + current() + " px"));
		}

		@Override
		protected void applyValue() {
			onChange.accept(current());
		}
	}
}

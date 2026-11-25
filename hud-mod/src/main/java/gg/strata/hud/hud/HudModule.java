package gg.strata.hud.hud;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;

import gg.strata.hud.StrataHud;
import gg.strata.hud.config.HudConfig;
import gg.strata.hud.render.Draw;

/**
 * One draggable overlay. Each frame the renderer calls {@link #layout} (work
 * out size and text), positions the module from its settings, then calls
 * {@link #draw} with (0,0) at the module's top-left corner.
 */
public abstract class HudModule {
	public static final int PAD = 4;

	public final String id;
	public final String name;
	protected int width;
	protected int height;

	/** Where it was last drawn, in GUI pixels. Used by the editor for hit-testing. */
	public int boxX, boxY, boxW, boxH;
	public boolean drawn;

	protected HudModule(String id, String name) {
		this.id = id;
		this.name = name;
	}

	public HudConfig.ModuleSettings settings() {
		return StrataHud.config().module(id);
	}

	/**
	 * Compute {@link #width}/{@link #height} for this frame. {@code editing}
	 * means the layout editor is open: show sample content so the module can
	 * be placed even when it would normally be empty. Leave width at 0 to skip.
	 */
	public abstract void layout(Minecraft mc, boolean editing);

	public abstract void draw(GuiGraphicsExtractor g, Minecraft mc, boolean editing);

	public int width() {
		return width;
	}

	public int height() {
		return height;
	}

	// ------------------------------------------------- shared text module

	/** A label in the muted colour followed by a value: "FPS 240". */
	public abstract static class Text extends HudModule {
		private String label = "";
		private String value = "";
		private int valueColor;

		protected Text(String id, String name) {
			super(id, name);
		}

		/** Return false to hide the module this frame. */
		protected abstract boolean update(Minecraft mc, boolean editing);

		protected final void set(String label, String value, int valueColor) {
			this.label = label;
			this.value = value;
			this.valueColor = valueColor;
		}

		@Override
		public void layout(Minecraft mc, boolean editing) {
			if (!update(mc, editing)) {
				width = 0;
				return;
			}
			int gap = label.isEmpty() ? 0 : 4;
			width = PAD * 2 + Draw.width(label) + gap + Draw.width(value);
			height = Draw.lineHeight() + 6;
		}

		@Override
		public void draw(GuiGraphicsExtractor g, Minecraft mc, boolean editing) {
			Draw.fill(g, 0, 0, width, height, Draw.background);
			int x = PAD;
			if (!label.isEmpty()) {
				Draw.text(g, label, x, 4, Draw.muted);
				x += Draw.width(label) + 4;
			}
			Draw.text(g, value, x, 4, valueColor == 0 ? Draw.text : valueColor);
		}
	}
}

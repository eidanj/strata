package gg.strata.hud.screen;

import java.util.ArrayList;
import java.util.List;

import com.mojang.blaze3d.platform.InputConstants;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.network.chat.Component;

import gg.strata.hud.StrataHud;
import gg.strata.hud.config.HudConfig;
import gg.strata.hud.hud.HudModule;
import gg.strata.hud.hud.HudRenderer;
import gg.strata.hud.render.Draw;

/**
 * Right Shift layout editor. The HUD keeps drawing underneath (with sample
 * content for empty modules); this screen adds outlines, dragging with
 * snapping, scroll-to-resize and right-click to show/hide.
 */
public final class HudEditorScreen extends Screen {
	private static final int SNAP = 4;
	private static final int GUIDE = 0xCCD3AE72;

	private static boolean open;

	private HudModule dragging;
	private HudModule selected;
	private int grabX;
	private int grabY;
	private final List<int[]> guides = new ArrayList<>(); // {x0, y0, x1, y1}

	public HudEditorScreen() {
		super(Component.literal("Arrange HUD"));
	}

	public static boolean isOpen() {
		return open;
	}

	@Override
	protected void init() {
		open = true;
		int y = this.height / 2 - 34;
		int cx = this.width / 2;
		addRenderableWidget(Button.builder(Component.literal("Crosshair"), b ->
				this.minecraft.gui.setScreen(new CrosshairScreen(this))).bounds(cx - 75, y, 150, 20).build());
		addRenderableWidget(Button.builder(Component.literal("Reset layout"), b -> resetLayout())
				.bounds(cx - 75, y + 24, 150, 20).build());
		addRenderableWidget(Button.builder(Component.literal("Done"), b -> onClose())
				.bounds(cx - 75, y + 48, 150, 20).build());
	}

	@Override
	public void removed() {
		open = false;
		StrataHud.save();
	}

	@Override
	public boolean isPauseScreen() {
		return false;
	}

	/** A light dim instead of the menu blur, so the HUD stays readable. */
	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		Draw.fill(g, 0, 0, this.width, this.height, 0x44000000);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		super.extractRenderState(g, mouseX, mouseY, delta);

		HudModule hover = dragging != null ? dragging : moduleAt(mouseX, mouseY);
		for (HudModule m : HudRenderer.MODULES) {
			if (!m.drawn) continue;
			boolean enabled = m.settings().enabled;
			if (!enabled) {
				Draw.fill(g, m.boxX, m.boxY, m.boxX + m.boxW, m.boxY + m.boxH, 0xB3161A1F);
				Draw.centeredText(g, "Hidden", m.boxX + m.boxW / 2, m.boxY + (m.boxH - 8) / 2, Draw.muted);
			}
			boolean hot = m == hover || m == selected;
			Draw.outline(g, m.boxX - 1, m.boxY - 1, m.boxW + 2, m.boxH + 2, hot ? Draw.accent : 0x66FFFFFF);
			if (m == hover) {
				String label = m.name + "  " + Math.round(m.settings().scale * 100) + "%";
				int ly = m.boxY - 11 < 2 ? m.boxY + m.boxH + 3 : m.boxY - 11;
				Draw.text(g, label, m.boxX, ly, Draw.accent);
			}
		}
		for (int[] line : guides) Draw.fill(g, line[0], line[1], line[2], line[3], GUIDE);

		Draw.centeredText(g, "Drag to move. Scroll to resize. Right-click to show or hide.",
				this.width / 2, this.height / 2 + 42, Draw.text);
		Draw.centeredText(g, "Arrow keys nudge the selected overlay. Right Shift or Esc closes.",
				this.width / 2, this.height / 2 + 54, Draw.muted);
	}

	// ---------------------------------------------------------------- input

	@Override
	public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
		if (super.mouseClicked(event, doubleClick)) return true;
		HudModule m = moduleAt(event.x(), event.y());
		if (m == null) {
			selected = null;
			return false;
		}
		selected = m;
		if (event.button() == InputConstants.MOUSE_BUTTON_RIGHT) {
			m.settings().enabled = !m.settings().enabled;
			return true;
		}
		if (event.button() == InputConstants.MOUSE_BUTTON_LEFT) {
			dragging = m;
			grabX = (int) event.x() - m.boxX;
			grabY = (int) event.y() - m.boxY;
			return true;
		}
		return false;
	}

	@Override
	public boolean mouseDragged(MouseButtonEvent event, double dx, double dy) {
		if (dragging == null) return super.mouseDragged(event, dx, dy);
		int x = (int) event.x() - grabX;
		int y = (int) event.y() - grabY;
		guides.clear();
		x = snapX(dragging, x);
		y = snapY(dragging, y);
		HudConfig.ModuleSettings s = dragging.settings();
		s.x = HudRenderer.toFraction(x, dragging.boxW, this.width);
		s.y = HudRenderer.toFraction(y, dragging.boxH, this.height);
		return true;
	}

	@Override
	public boolean mouseReleased(MouseButtonEvent event) {
		if (dragging != null) {
			dragging = null;
			guides.clear();
			return true;
		}
		return super.mouseReleased(event);
	}

	@Override
	public boolean mouseScrolled(double mouseX, double mouseY, double horizontal, double vertical) {
		HudModule m = moduleAt(mouseX, mouseY);
		if (m == null) return super.mouseScrolled(mouseX, mouseY, horizontal, vertical);
		HudConfig.ModuleSettings s = m.settings();
		s.scale = Math.round(Math.max(0.5, Math.min(2.5, s.scale + Math.signum(vertical) * 0.05)) * 100) / 100.0;
		return true;
	}

	@Override
	public boolean keyPressed(KeyEvent event) {
		if (StrataHud.EDITOR_KEY.matches(event)) {
			onClose();
			return true;
		}
		if (selected != null && selected.drawn) {
			int step = event.hasShiftDown() ? 5 : 1;
			int dx = 0;
			int dy = 0;
			int key = event.key();
			if (key == InputConstants.KEY_LEFT) dx = -step;
			else if (key == InputConstants.KEY_RIGHT) dx = step;
			else if (key == InputConstants.KEY_UP) dy = -step;
			else if (key == InputConstants.KEY_DOWN) dy = step;
			if (dx != 0 || dy != 0) {
				HudConfig.ModuleSettings s = selected.settings();
				s.x = HudRenderer.toFraction(selected.boxX + dx, selected.boxW, this.width);
				s.y = HudRenderer.toFraction(selected.boxY + dy, selected.boxH, this.height);
				return true;
			}
		}
		return super.keyPressed(event);
	}

	// -------------------------------------------------------------- helpers

	private HudModule moduleAt(double x, double y) {
		List<HudModule> list = HudRenderer.MODULES;
		for (int i = list.size() - 1; i >= 0; i--) {
			HudModule m = list.get(i);
			if (m.drawn && x >= m.boxX && x < m.boxX + m.boxW && y >= m.boxY && y < m.boxY + m.boxH) return m;
		}
		return null;
	}

	/** A position to snap to and where to draw its guide line. */
	private record Target(int pos, int guide) {
	}

	/** Snap to the screen edges, the centre line, and other overlays' edges. */
	private int snapX(HudModule m, int x) {
		int w = m.boxW;
		int right = this.width - HudRenderer.MARGIN;
		List<Target> targets = new ArrayList<>(List.of(
				new Target(HudRenderer.MARGIN, HudRenderer.MARGIN),
				new Target(right - w, right),
				new Target(this.width / 2 - w / 2, this.width / 2)));
		for (HudModule o : HudRenderer.MODULES) {
			if (o == m || !o.drawn) continue;
			targets.add(new Target(o.boxX, o.boxX)); // left edges line up
			targets.add(new Target(o.boxX + o.boxW - w, o.boxX + o.boxW)); // right edges line up
		}
		for (Target t : targets) {
			if (Math.abs(x - t.pos()) <= SNAP) {
				guides.add(new int[] {t.guide(), 0, t.guide() + 1, this.height});
				return t.pos();
			}
		}
		return x;
	}

	private int snapY(HudModule m, int y) {
		int h = m.boxH;
		int bottom = this.height - HudRenderer.MARGIN;
		List<Target> targets = new ArrayList<>(List.of(
				new Target(HudRenderer.MARGIN, HudRenderer.MARGIN),
				new Target(bottom - h, bottom),
				new Target(this.height / 2 - h / 2, this.height / 2)));
		for (HudModule o : HudRenderer.MODULES) {
			if (o == m || !o.drawn) continue;
			targets.add(new Target(o.boxY, o.boxY));
			targets.add(new Target(o.boxY + o.boxH - h, o.boxY + o.boxH));
			targets.add(new Target(o.boxY + o.boxH + 2, o.boxY + o.boxH + 1)); // stack directly below
		}
		for (Target t : targets) {
			if (Math.abs(y - t.pos()) <= SNAP) {
				guides.add(new int[] {0, t.guide(), this.width, t.guide() + 1});
				return t.pos();
			}
		}
		return y;
	}

	private void resetLayout() {
		HudConfig fresh = HudConfig.defaultConfig();
		HudConfig current = StrataHud.config();
		current.modules = fresh.modules;
	}
}

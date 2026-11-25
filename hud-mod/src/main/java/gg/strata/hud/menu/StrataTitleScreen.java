package gg.strata.hud.menu;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;

import gg.strata.hud.StrataHud;
import gg.strata.hud.config.HudConfig;
import gg.strata.hud.render.Draw;
import gg.strata.hud.screen.CrosshairScreen;

/**
 * Strata's main menu. Replaces the vanilla title screen (see StrataHud) and
 * can be switched back with "Use classic menu".
 *
 *  ┌──────────────┬───────────────────────────────┐
 *  │ ▤ STRATA     │                               │
 *  │ 26.3 Fabric  │          dark sky             │
 *  │              │                               │
 *  │ Singleplayer │  ~~~~~~~ drifting strata ~~~~ │
 *  │ Multiplayer  │  ~~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
 *  │ Options      │                               │
 *  │ HUD + cross  │                               │
 *  │ Quit game    │                               │
 *  │ Signed in as │                     Strata 0.1 │
 *  │ Classic menu │          Copyright Mojang AB… │
 *  └──────────────┴───────────────────────────────┘
 */
public final class StrataTitleScreen extends Screen {
	private static final int PANEL = 170;
	private static final int HEADER = 62;
	private static final int FOOTER = 40;
	private static final int STEP = 22;
	/** Height of the button stack: five 20 px buttons, 22 px apart, plus 4 px before Quit. */
	private static final int STACK = STEP * 4 + 4 + 20;
	private static final float RISE_SECONDS = 0.7f;
	private final long openedAt = System.nanoTime();
	private String version = "";

	public StrataTitleScreen() {
		super(Component.literal("Strata"));
	}

	@Override
	protected void init() {
		version = "Minecraft " + Menus.version("minecraft") + " with Fabric";
		int x = 18;
		int w = PANEL - 36;

		// Centre the buttons between the logo and the footer; at the smallest
		// GUI size (240 px tall) they still clear both.
		int top = HEADER;
		int bottom = this.height - FOOTER;
		int y = top + Math.max(0, (bottom - top - STACK) / 2);

		button(x, y, w, "Singleplayer", () -> this.minecraft.gui.setScreen(Menus.singleplayer(this)));
		button(x, y += STEP, w, "Multiplayer", () -> this.minecraft.gui.setScreen(Menus.multiplayer(this)));
		button(x, y += STEP, w, "Options", () -> this.minecraft.gui.setScreen(Menus.options(this)));
		button(x, y += STEP, w, "Crosshair", () -> this.minecraft.gui.setScreen(new CrosshairScreen(this)));
		button(x, y += STEP + 4, w, "Quit game", () -> this.minecraft.stop());

		addRenderableWidget(new MenuButton(x, this.height - 16, 90, 11, Component.literal("Use classic menu"), b -> {
			StrataHud.config().menu.enabled = false;
			StrataHud.save();
			this.minecraft.gui.setScreen(new TitleScreen());
		}, true));
	}

	private void button(int x, int y, int w, String label, Runnable action) {
		addRenderableWidget(new MenuButton(x, y, w, 20, Component.literal(label), b -> action.run(), false));
	}

	@Override
	public boolean shouldCloseOnEsc() {
		return false;
	}

	@Override
	public void extractBackground(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		HudConfig.Menu menu = StrataHud.config().menu;
		float t = (System.nanoTime() - openedAt) / 1_000_000_000f;
		float rise = menu.motion ? easeOut(Math.min(1f, t / RISE_SECONDS)) : 1f;
		StrataBackground.draw(g, this.width, this.height, menu.motion ? mouseX : this.width / 2, rise, menu.motion ? t : 0f);

		// Side panel with a thin accent edge.
		Draw.fill(g, 0, 0, PANEL, this.height, 0xEE161A1F);
		Draw.fill(g, PANEL, 0, PANEL + 1, this.height, 0x809A86B5);
	}

	@Override
	public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float delta) {
		super.extractRenderState(g, mouseX, mouseY, delta);

		// Logo: the mark plus a spaced, double-size wordmark.
		StrataBackground.mark(g, 18, 20, 18);
		Draw.push(g);
		Draw.translate(g, 44, 21);
		Draw.scale(g, 2f);
		int cx = 0;
		for (char c : "STRATA".toCharArray()) {
			String s = String.valueOf(c);
			Draw.text(g, s, cx, 0, Draw.text, Draw.shadow);
			cx += Draw.width(s) + 1;
		}
		Draw.pop(g);
		Draw.text(g, version, 18, 44, Draw.muted, false);

		String signedIn = "Signed in as ";
		Draw.text(g, signedIn, 18, this.height - 30, Draw.muted, false);
		Draw.text(g, Menus.playerName(), 18 + Draw.width(signedIn), this.height - 30, Draw.text, Draw.shadow);

		String copyright = "Copyright Mojang AB. Do not distribute!";
		Draw.text(g, copyright, this.width - Draw.width(copyright) - 6, this.height - 12, 0xB3E6E3DC, false);
		String strata = "Strata " + Menus.version("strata-hud");
		Draw.text(g, strata, this.width - Draw.width(strata) - 6, this.height - 23, 0x809AA3AD, false);
	}

	private static float easeOut(float t) {
		float u = 1f - t;
		return 1f - u * u * u;
	}
}

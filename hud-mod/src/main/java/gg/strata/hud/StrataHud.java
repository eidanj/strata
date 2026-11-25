package gg.strata.hud;

import org.lwjgl.sdl.SDLScancode;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import com.mojang.blaze3d.platform.InputConstants;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;

import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.fabricmc.fabric.api.client.rendering.v1.hud.VanillaHudElements;

import gg.strata.hud.config.HudConfig;
import gg.strata.hud.hud.HudRenderer;
import gg.strata.hud.render.CrosshairRenderer;
import gg.strata.hud.render.Draw;
import gg.strata.hud.render.Game;
import gg.strata.hud.screen.HudEditorScreen;

public final class StrataHud implements ClientModInitializer {
	public static final String MOD_ID = "strata-hud";
	public static final Logger LOGGER = LoggerFactory.getLogger(MOD_ID);

	private static HudConfig config = new HudConfig();

	public static final KeyMapping.Category CATEGORY =
			KeyMapping.Category.register(Identifier.fromNamespaceAndPath("strata", "hud"));

	/**
	 * Right Shift opens the layout editor. 26.3 moved input to SDL, so keys are
	 * SDL scancodes; players can rebind this under Controls.
	 */
	public static final KeyMapping EDITOR_KEY = KeyMappingHelper.registerKeyMapping(new KeyMapping(
			"key.strata.editor",
			InputConstants.Type.KEYBOARD,
			SDLScancode.SDL_SCANCODE_RSHIFT,
			CATEGORY));

	@Override
	public void onInitializeClient() {
		reload();

		HudElementRegistry.attachElementBefore(
				VanillaHudElements.CHAT,
				Identifier.fromNamespaceAndPath("strata", "overlays"),
				HudRenderer::extract);

		HudElementRegistry.replaceElement(
				VanillaHudElements.CROSSHAIR,
				vanilla -> (graphics, delta) -> CrosshairRenderer.extract(graphics, delta, vanilla));

		ClientLifecycleEvents.CLIENT_STARTED.register(StrataHud::applyGameOptions);

		ClientTickEvents.END_CLIENT_TICK.register(mc -> {
			while (EDITOR_KEY.consumeClick()) {
				if (mc.player != null) mc.gui.setScreen(new HudEditorScreen());
			}
		});

		LOGGER.info("Strata HUD ready. Press Right Shift in game to arrange it.");
	}

	public static HudConfig config() {
		return config;
	}

	/** Re-read the file, e.g. after the launcher changed it. */
	public static void reload() {
		config = HudConfig.load();
		Draw.applyTheme(config.theme);
	}

	public static void replace(HudConfig fresh) {
		config = fresh;
		Draw.applyTheme(config.theme);
	}

	public static void save() {
		config.save();
		applyGameOptions(Minecraft.getInstance());
	}

	/** ToggleSprint rides on the vanilla toggle option; switch it on with the module. */
	public static void applyGameOptions(Minecraft mc) {
		if (mc == null || mc.options == null) return;
		if (config.module("togglesprint").enabled && !Game.toggleSprint(mc)) {
			Game.setToggleSprint(mc, true);
		}
	}
}

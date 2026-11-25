package gg.strata.hud.menu;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.multiplayer.JoinMultiplayerScreen;
import net.minecraft.client.gui.screens.options.OptionsScreen;
import net.minecraft.client.gui.screens.worldselection.SelectWorldScreen;

import net.fabricmc.loader.api.FabricLoader;

/**
 * The vanilla screens the Strata menu opens. Like Draw and Game, this keeps
 * the version-sensitive names in one place.
 */
public final class Menus {
	private Menus() {
	}

	public static Screen singleplayer(Screen parent) {
		return new SelectWorldScreen(parent);
	}

	public static Screen multiplayer(Screen parent) {
		return new JoinMultiplayerScreen(parent);
	}

	public static Screen options(Screen parent) {
		return new OptionsScreen(parent, Minecraft.getInstance().options);
	}

	public static String playerName() {
		return Minecraft.getInstance().getUser().getName();
	}

	/** Version of a loaded mod, e.g. "minecraft" -> "26.3". */
	public static String version(String modId) {
		return FabricLoader.getInstance()
				.getModContainer(modId)
				.map(c -> c.getMetadata().getVersion().getFriendlyString())
				.orElse("");
	}
}

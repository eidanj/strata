package gg.strata.hud.render;

import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.multiplayer.ClientPacketListener;
import net.minecraft.client.multiplayer.PlayerInfo;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.phys.HitResult;

/**
 * Game-state reads that tend to move between versions, kept in one place
 * like {@link Draw}. These are long-standing APIs, but 26.x renames a lot.
 */
public final class Game {
	private Game() {
	}

	/** Latency to the server in ms, or -1 when unknown (e.g. still joining). */
	public static int ping(Minecraft mc) {
		ClientPacketListener conn = mc.getConnection();
		if (conn == null || mc.player == null) return -1;
		PlayerInfo info = conn.getPlayerInfo(mc.player.getUUID());
		return info == null ? -1 : info.getLatency();
	}

	public static boolean singleplayer(Minecraft mc) {
		return mc.hasSingleplayerServer();
	}

	/** GUI pixels per screen pixel. */
	public static float guiScale(Minecraft mc) {
		return (float) mc.getWindow().getGuiScale();
	}

	public static boolean firstPerson(Minecraft mc) {
		return mc.options.getCameraType().isFirstPerson();
	}

	public static boolean aimingAtEntity(Minecraft mc) {
		return mc.hitResult != null && mc.hitResult.getType() == HitResult.Type.ENTITY;
	}

	/** 0..1, where 1 means the next hit does full damage. */
	public static float attackCharge(LocalPlayer player) {
		return player.getAttackStrengthScale(0.0F);
	}

	/** Vanilla's "Sprint: Toggle" option. Strata's ToggleSprint builds on it. */
	public static void setToggleSprint(Minecraft mc, boolean on) {
		mc.options.toggleSprint().set(on);
	}

	public static boolean toggleSprint(Minecraft mc) {
		return mc.options.toggleSprint().get();
	}

	/** The key's current binding as shown in Controls, shortened for a keycap. */
	public static String keyLabel(KeyMapping key, String fallback) {
		try {
			String s = key.getTranslatedKeyMessage().getString();
			if (s.isEmpty()) return fallback;
			return s.length() <= 3 ? s.toUpperCase() : s.substring(0, 3);
		} catch (RuntimeException e) {
			return fallback;
		}
	}
}

package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;

import gg.strata.hud.hud.HudModule;

public final class CoordsModule extends HudModule.Text {
	public CoordsModule() {
		super("coords", "Coordinates");
	}

	@Override
	protected boolean update(Minecraft mc, boolean editing) {
		if (mc.player == null) return false;
		String facing = switch (mc.player.getDirection()) {
			case NORTH -> "N";
			case SOUTH -> "S";
			case EAST -> "E";
			case WEST -> "W";
			default -> "";
		};
		set("XYZ", String.format("%d %d %d  %s",
				(int) Math.floor(mc.player.getX()),
				(int) Math.floor(mc.player.getY()),
				(int) Math.floor(mc.player.getZ()),
				facing), 0);
		return true;
	}
}

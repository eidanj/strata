package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;

import gg.strata.hud.hud.HudModule;
import gg.strata.hud.hud.InputStats;

public final class FpsModule extends HudModule.Text {
	public FpsModule() {
		super("fps", "FPS");
	}

	@Override
	protected boolean update(Minecraft mc, boolean editing) {
		set("FPS", Integer.toString(InputStats.fps()), 0);
		return true;
	}
}

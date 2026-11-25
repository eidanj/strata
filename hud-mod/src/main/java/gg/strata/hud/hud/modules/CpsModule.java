package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;

import gg.strata.hud.hud.HudModule;
import gg.strata.hud.hud.InputStats;

/** Left | right clicks in the last second. */
public final class CpsModule extends HudModule.Text {
	public CpsModule() {
		super("cps", "CPS");
	}

	@Override
	protected boolean update(Minecraft mc, boolean editing) {
		set("CPS", InputStats.leftCps() + " | " + InputStats.rightCps(), 0);
		return true;
	}
}

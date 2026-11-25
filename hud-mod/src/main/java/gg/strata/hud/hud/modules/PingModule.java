package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;

import gg.strata.hud.config.Colors;
import gg.strata.hud.hud.HudModule;
import gg.strata.hud.render.Game;

public final class PingModule extends HudModule.Text {
	public PingModule() {
		super("ping", "Ping");
	}

	@Override
	protected boolean update(Minecraft mc, boolean editing) {
		if (Game.singleplayer(mc)) {
			set("Ping", "local", 0);
			return true;
		}
		int ms = Game.ping(mc);
		if (ms < 0) {
			set("Ping", "–", 0);
			return true;
		}
		int color = ms < 80 ? 0 : ms < 160 ? Colors.WARN : Colors.BAD;
		set("Ping", ms + " ms", color);
		return true;
	}
}

package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;

import gg.strata.hud.hud.HudModule;
import gg.strata.hud.render.Draw;
import gg.strata.hud.render.Game;

/**
 * Builds on vanilla's "Sprint: Toggle" option (turned on when this module is
 * enabled, see StrataHud#applyGameOptions), so it behaves exactly like the
 * game's own toggle and is safe on any server. This module shows the state.
 */
public final class ToggleSprintModule extends HudModule.Text {
	public ToggleSprintModule() {
		super("togglesprint", "Toggle sprint");
	}

	@Override
	protected boolean update(Minecraft mc, boolean editing) {
		if (mc.player == null) return false;
		if (mc.player.isShiftKeyDown()) {
			set("", "Sneaking", Draw.muted);
		} else if (Game.toggleSprint(mc) && mc.options.keySprint.isDown()) {
			set("", "Sprinting (toggled)", Draw.accent);
		} else if (mc.player.isSprinting()) {
			set("", "Sprinting", 0);
		} else if (editing) {
			set("", "Sprinting (toggled)", Draw.accent);
		} else {
			return false;
		}
		return true;
	}
}

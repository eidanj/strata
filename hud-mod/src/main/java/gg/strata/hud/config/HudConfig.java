package gg.strata.hud.config;

import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.LinkedHashMap;
import java.util.Map;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonElement;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;

import net.fabricmc.loader.api.FabricLoader;

import gg.strata.hud.StrataHud;

/**
 * The layout file shared with the launcher: {@code config/strata-hud.json}.
 * Defaults come from {@code strata-hud.default.json} (copied from the repo's
 * {@code shared/} folder at build time), and the saved file is laid over them.
 */
public final class HudConfig {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().disableHtmlEscaping().create();
	private static final String DEFAULT_RESOURCE = "/strata-hud.default.json";

	public int version = 1;
	public Theme theme = new Theme();
	public Crosshair crosshair = new Crosshair();
	public Map<String, ModuleSettings> modules = new LinkedHashMap<>();

	public static final class Theme {
		public String text = "#FFE6E3DC";
		public String muted = "#FF9AA3AD";
		public String accent = "#FFD3AE72";
		public String background = "#99161A1F";
		public String pressed = "#E6E6E3DC";
		public String pressedText = "#FF161A1F";
		public boolean textShadow = true;
	}

	public static final class Crosshair {
		public boolean enabled = true;
		public String style = "plus";
		public int length = 6;
		public int gap = 3;
		public int thickness = 2;
		public boolean dot = false;
		public int dotSize = 2;
		public boolean outline = true;
		public String color = "#FFFFFFFF";
		public String outlineColor = "#C8000000";
		public boolean colorOnTarget = true;
		public String targetColor = "#FFFF5A5A";
		public boolean cooldownIndicator = true;
	}

	/** Position is a fraction (0..1) of the free space, so layouts survive resolution changes. */
	public static final class ModuleSettings {
		public boolean enabled = true;
		public double x;
		public double y;
		public double scale = 1.0;
		// Module-specific options. Null means "not used by this module" and isn't written.
		public String layout;
		public String durability;
		public Boolean showMouse;
		public Boolean showSpace;
		public Boolean showCount;
	}

	public ModuleSettings module(String id) {
		return modules.computeIfAbsent(id, k -> new ModuleSettings());
	}

	// ------------------------------------------------------------------ io

	public static Path file() {
		return FabricLoader.getInstance().getConfigDir().resolve("strata-hud.json");
	}

	public static HudConfig load() {
		JsonObject merged = defaults();
		Path path = file();
		if (Files.exists(path)) {
			try {
				JsonElement saved = JsonParser.parseString(Files.readString(path, StandardCharsets.UTF_8));
				if (saved.isJsonObject()) merge(merged, saved.getAsJsonObject());
			} catch (Exception e) {
				StrataHud.LOGGER.warn("Couldn't read {}, using defaults", path, e);
			}
		}
		return GSON.fromJson(merged, HudConfig.class);
	}

	public static HudConfig defaultConfig() {
		return GSON.fromJson(defaults(), HudConfig.class);
	}

	public void save() {
		Path path = file();
		try {
			Files.createDirectories(path.getParent());
			Path tmp = path.resolveSibling("strata-hud.json.tmp");
			Files.writeString(tmp, GSON.toJson(this), StandardCharsets.UTF_8);
			Files.move(tmp, path, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
		} catch (IOException e) {
			StrataHud.LOGGER.error("Couldn't save {}", path, e);
		}
	}

	private static JsonObject defaults() {
		try (InputStream in = HudConfig.class.getResourceAsStream(DEFAULT_RESOURCE)) {
			if (in != null) {
				return JsonParser.parseReader(new InputStreamReader(in, StandardCharsets.UTF_8)).getAsJsonObject();
			}
		} catch (Exception e) {
			StrataHud.LOGGER.warn("Bundled default HUD config is unreadable", e);
		}
		return GSON.toJsonTree(new HudConfig()).getAsJsonObject();
	}

	private static void merge(JsonObject base, JsonObject over) {
		for (Map.Entry<String, JsonElement> e : over.entrySet()) {
			JsonElement existing = base.get(e.getKey());
			if (existing != null && existing.isJsonObject() && e.getValue().isJsonObject()) {
				merge(existing.getAsJsonObject(), e.getValue().getAsJsonObject());
			} else {
				base.add(e.getKey(), e.getValue());
			}
		}
	}
}

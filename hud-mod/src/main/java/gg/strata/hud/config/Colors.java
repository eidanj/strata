package gg.strata.hud.config;

/** Colours are stored as "#AARRGGBB" (or "#RRGGBB") and drawn as ARGB ints. */
public final class Colors {
	public static final int OK = 0xFF93B17D;
	public static final int WARN = 0xFFD3AE72;
	public static final int BAD = 0xFFD9735F;

	private Colors() {
	}

	public static int parse(String value, int fallback) {
		if (value == null) return fallback;
		String v = value.startsWith("#") ? value.substring(1) : value;
		try {
			if (v.length() == 6) return 0xFF000000 | Integer.parseUnsignedInt(v, 16);
			if (v.length() == 8) return (int) Long.parseLong(v, 16);
		} catch (NumberFormatException ignored) {
			// fall through
		}
		return fallback;
	}

	public static String format(int argb) {
		return String.format("#%08X", argb);
	}

	/** Green above 60% durability, amber above 30%, red below. */
	public static int durability(float fraction) {
		if (fraction > 0.6f) return OK;
		if (fraction > 0.3f) return WARN;
		return BAD;
	}

	public static int lerp(int from, int to, float t) {
		t = Math.max(0f, Math.min(1f, t));
		int a = (int) (((from >>> 24) & 0xFF) + (((to >>> 24) & 0xFF) - ((from >>> 24) & 0xFF)) * t);
		int r = (int) (((from >> 16) & 0xFF) + (((to >> 16) & 0xFF) - ((from >> 16) & 0xFF)) * t);
		int g = (int) (((from >> 8) & 0xFF) + (((to >> 8) & 0xFF) - ((from >> 8) & 0xFF)) * t);
		int b = (int) ((from & 0xFF) + ((to & 0xFF) - (from & 0xFF)) * t);
		return (a << 24) | (r << 16) | (g << 8) | b;
	}
}

package gg.strata.hud.hud;

import net.minecraft.client.Minecraft;

/**
 * Frame rate and clicks per second, measured by the HUD itself so nothing
 * depends on vanilla's debug counters. Called once per rendered frame.
 */
public final class InputStats {
	private static long windowStart;
	private static int frames;
	private static int fps;

	private static final ClickWindow LEFT = new ClickWindow();
	private static final ClickWindow RIGHT = new ClickWindow();
	private static boolean leftWasDown;
	private static boolean rightWasDown;

	private InputStats() {
	}

	public static void frame(Minecraft mc) {
		long now = System.nanoTime();
		frames++;
		if (windowStart == 0) windowStart = now;
		long elapsed = now - windowStart;
		if (elapsed >= 500_000_000L) {
			fps = Math.round(frames * 1_000_000_000f / elapsed);
			frames = 0;
			windowStart = now;
		}

		// Count presses on the rising edge. Polling every frame catches
		// clicks far faster than any human, even at 60 FPS.
		boolean left = mc.options.keyAttack.isDown();
		boolean right = mc.options.keyUse.isDown();
		if (left && !leftWasDown) LEFT.add(now);
		if (right && !rightWasDown) RIGHT.add(now);
		leftWasDown = left;
		rightWasDown = right;
	}

	public static int fps() {
		return fps;
	}

	public static int leftCps() {
		return LEFT.count(System.nanoTime());
	}

	public static int rightCps() {
		return RIGHT.count(System.nanoTime());
	}

	/** Timestamps of the last 64 clicks in a ring buffer; no allocation per click. */
	private static final class ClickWindow {
		private final long[] times = new long[64];
		private int head;

		void add(long t) {
			times[head] = t;
			head = (head + 1) % times.length;
		}

		int count(long now) {
			int n = 0;
			for (long t : times) {
				if (t != 0 && now - t <= 1_000_000_000L) n++;
			}
			return n;
		}
	}
}

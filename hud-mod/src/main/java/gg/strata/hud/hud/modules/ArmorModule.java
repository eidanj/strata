package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.world.entity.EquipmentSlot;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

import gg.strata.hud.config.Colors;
import gg.strata.hud.hud.HudModule;
import gg.strata.hud.render.Draw;

/** Helmet to boots, each with its durability. Vertical or horizontal. */
public final class ArmorModule extends HudModule {
	private static final EquipmentSlot[] SLOTS = {
		EquipmentSlot.HEAD, EquipmentSlot.CHEST, EquipmentSlot.LEGS, EquipmentSlot.FEET,
	};
	private static final int ROW = 18;

	private final ItemStack[] stacks = new ItemStack[4];
	private final String[] labels = new String[4];
	private final int[] colors = new int[4];
	private int count;
	private int textWidth;

	public ArmorModule() {
		super("armor", "Armor status");
	}

	private boolean horizontal() {
		return "horizontal".equals(settings().layout);
	}

	@Override
	public void layout(Minecraft mc, boolean editing) {
		count = 0;
		textWidth = 0;
		if (mc.player == null) {
			width = 0;
			return;
		}
		for (int i = 0; i < SLOTS.length; i++) {
			ItemStack stack = mc.player.getItemBySlot(SLOTS[i]);
			if (stack.isEmpty() && editing) stack = sample(i);
			if (stack.isEmpty()) continue;
			stacks[count] = stack;
			labels[count] = durabilityText(stack, settings().durability);
			colors[count] = durabilityColor(stack);
			textWidth = Math.max(textWidth, Draw.width(labels[count]));
			count++;
		}
		if (count == 0) {
			width = 0;
			return;
		}
		if (horizontal()) {
			int cell = Math.max(16, textWidth) + 4;
			width = cell * count;
			height = 16 + 2 + Draw.lineHeight();
		} else {
			width = 16 + (textWidth > 0 ? 4 + textWidth : 0);
			height = ROW * count - 2;
		}
	}

	@Override
	public void draw(GuiGraphicsExtractor g, Minecraft mc, boolean editing) {
		boolean horiz = horizontal();
		int cell = Math.max(16, textWidth) + 4;
		for (int i = 0; i < count; i++) {
			if (horiz) {
				int x = i * cell + (cell - 16) / 2;
				Draw.item(g, stacks[i], x, 0, "");
				Draw.text(g, labels[i], i * cell + (cell - Draw.width(labels[i])) / 2, 18, colors[i]);
			} else {
				int y = i * ROW;
				Draw.item(g, stacks[i], 0, y, "");
				Draw.text(g, labels[i], 20, y + 4, colors[i]);
			}
		}
	}

	/** Shown in the layout editor when the slot is empty. */
	private static ItemStack sample(int slot) {
		ItemStack s = switch (slot) {
			case 0 -> new ItemStack(Items.DIAMOND_HELMET);
			case 1 -> new ItemStack(Items.DIAMOND_CHESTPLATE);
			case 2 -> new ItemStack(Items.DIAMOND_LEGGINGS);
			default -> new ItemStack(Items.DIAMOND_BOOTS);
		};
		s.setDamageValue((int) (s.getMaxDamage() * (0.1 + slot * 0.22)));
		return s;
	}

	static String durabilityText(ItemStack stack, String mode) {
		if (!stack.isDamageableItem()) return "";
		int max = stack.getMaxDamage();
		int left = max - stack.getDamageValue();
		if ("remaining".equals(mode)) return Integer.toString(left);
		return Math.round(left * 100f / max) + "%";
	}

	static int durabilityColor(ItemStack stack) {
		if (!stack.isDamageableItem()) return Draw.text;
		int max = stack.getMaxDamage();
		return Colors.durability((max - stack.getDamageValue()) / (float) max);
	}
}

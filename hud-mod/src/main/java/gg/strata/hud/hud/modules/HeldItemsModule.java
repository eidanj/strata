package gg.strata.hud.hud.modules;

import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.player.LocalPlayer;
import net.minecraft.world.entity.player.Inventory;
import net.minecraft.world.item.ItemStack;
import net.minecraft.world.item.Items;

import gg.strata.hud.hud.HudModule;
import gg.strata.hud.render.Draw;

/**
 * Main-hand and off-hand items. Tools and weapons show durability; stackable
 * items (pearls, blocks, arrows) show how many you carry in total.
 */
public final class HeldItemsModule extends HudModule {
	private static final int ROW = 18;

	private final ItemStack[] stacks = new ItemStack[2];
	private final String[] labels = new String[2];
	private final int[] colors = new int[2];
	private int count;

	public HeldItemsModule() {
		super("held", "Held items");
	}

	@Override
	public void layout(Minecraft mc, boolean editing) {
		LocalPlayer p = mc.player;
		count = 0;
		if (p == null) {
			width = 0;
			return;
		}
		int textWidth = 0;
		ItemStack[] hands = {p.getMainHandItem(), p.getOffhandItem()};
		for (int i = 0; i < 2; i++) {
			ItemStack stack = hands[i];
			if (stack.isEmpty() && editing) stack = i == 0 ? sampleSword() : new ItemStack(Items.ENDER_PEARL, 16);
			if (stack.isEmpty()) continue;
			stacks[count] = stack;
			if (stack.isDamageableItem()) {
				labels[count] = ArmorModule.durabilityText(stack, settings().durability);
				colors[count] = ArmorModule.durabilityColor(stack);
			} else if (showCount() && stack.getMaxStackSize() > 1) {
				labels[count] = Integer.toString(editing && hands[i].isEmpty() ? stack.getCount() : carried(p, stack));
				colors[count] = Draw.text;
			} else {
				labels[count] = "";
				colors[count] = Draw.text;
			}
			textWidth = Math.max(textWidth, Draw.width(labels[count]));
			count++;
		}
		if (count == 0) {
			width = 0;
			return;
		}
		width = 16 + (textWidth > 0 ? 4 + textWidth : 0);
		height = ROW * count - 2;
	}

	@Override
	public void draw(GuiGraphicsExtractor g, Minecraft mc, boolean editing) {
		for (int i = 0; i < count; i++) {
			int y = i * ROW;
			// Empty count text: the total is drawn beside the icon instead.
			Draw.item(g, stacks[i], 0, y, "");
			Draw.text(g, labels[i], 20, y + 4, colors[i]);
		}
	}

	private boolean showCount() {
		Boolean b = settings().showCount;
		return b == null || b;
	}

	private static int carried(LocalPlayer p, ItemStack like) {
		Inventory inv = p.getInventory();
		int total = 0;
		for (int i = 0; i < inv.getContainerSize(); i++) {
			ItemStack s = inv.getItem(i);
			if (!s.isEmpty() && ItemStack.isSameItem(s, like)) total += s.getCount();
		}
		return total;
	}

	private static ItemStack sampleSword() {
		ItemStack s = new ItemStack(Items.DIAMOND_SWORD);
		s.setDamageValue(s.getMaxDamage() / 3);
		return s;
	}
}

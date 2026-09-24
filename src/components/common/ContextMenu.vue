<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";

export interface MenuItem {
  label?: string;
  icon?: any;
  danger?: boolean;
  divider?: boolean;
  disabled?: boolean;
  onClick?: () => void;
}

const visible = ref(false);
const x = ref(0);
const y = ref(0);
const items = ref<MenuItem[]>([]);

function open(event: MouseEvent, menu: MenuItem[]) {
  event.preventDefault();
  event.stopPropagation();
  items.value = menu;
  // 预判边界：若靠近右/下边则反向
  const maxX = window.innerWidth - 180;
  const maxY = window.innerHeight - menu.length * 30 - 10;
  x.value = Math.min(event.clientX, maxX);
  y.value = Math.min(event.clientY, Math.max(maxY, 0));
  visible.value = true;
}

function hide() {
  visible.value = false;
}

function handleClick(item: MenuItem) {
  if (item.disabled || item.divider) return;
  hide();
  item.onClick?.();
}

onMounted(() => {
  window.addEventListener("click", hide);
  window.addEventListener("blur", hide);
});
onBeforeUnmount(() => {
  window.removeEventListener("click", hide);
  window.removeEventListener("blur", hide);
});

defineExpose({ open, hide });
</script>

<template>
  <div
    v-if="visible"
    class="fixed z-40 min-w-[160px] py-1 pop text-[13px] menu-enter"
    :style="{ left: x + 'px', top: y + 'px' }"
    @click.stop
  >
    <template v-for="(item, idx) in items" :key="idx">
      <div v-if="item.divider" class="my-1 border-t border-app-border" />
      <button
        v-else
        :disabled="item.disabled"
        class="w-full flex items-center gap-2 px-3 h-7 text-left rounded-sm hover:bg-app-hover disabled:opacity-40 disabled:hover:bg-transparent transition-colors"
        :class="item.danger ? 'text-accent-red hover:bg-accent-red/10' : 'text-app-text'"
        @click="handleClick(item)"
      >
        <component v-if="item.icon" :is="item.icon" :size="14" class="text-app-muted" />
        <span>{{ item.label }}</span>
      </button>
    </template>
  </div>
</template>

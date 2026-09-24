<script setup lang="ts">
import { computed } from "vue";
import { ChevronDown, Settings2, Check } from "lucide-vue-next";
import { Menu, MenuButton, MenuItems, MenuItem } from "@headlessui/vue";
import { useEnvironmentStore } from "@/stores/environment";

const props = defineProps<{ onManage: () => void }>();
const envStore = useEnvironmentStore();

const activeName = computed(() => envStore.active?.name ?? "无环境");

async function select(id: string) {
  await envStore.setActive(id);
}
</script>

<template>
  <Menu as="div" class="relative">
    <MenuButton
      class="w-full flex items-center gap-2 px-2 h-7 rounded-md hover:bg-app-hover text-xs bg-app-bg border border-app-border/60 hover:border-app-border transition-colors"
    >
      <span class="w-1.5 h-1.5 rounded-full bg-accent-green flex-shrink-0 shadow-[0_0_6px_rgb(var(--accent-green)/0.6)]" />
      <span class="flex-1 truncate text-left text-app-text/90">{{ activeName }}</span>
      <ChevronDown :size="13" class="text-app-muted flex-shrink-0" />
    </MenuButton>
    <MenuItems
      class="absolute left-0 right-0 mt-1 z-40 py-1 pop menu-enter"
    >
      <div class="px-3 py-1 text-[10px] text-app-muted uppercase tracking-[0.08em]">环境</div>
      <MenuItem v-for="env in envStore.environments" :key="env.id" v-slot="{ active }">
        <button
          class="w-full flex items-center gap-2 px-3 h-7 text-left text-xs"
          :class="active ? 'bg-app-hover' : ''"
          @click="select(env.id)"
        >
          <span
            class="w-1.5 h-1.5 rounded-full flex-shrink-0"
            :class="env.isActive ? 'bg-accent-green' : 'bg-app-border'"
          />
          <span class="flex-1 truncate">{{ env.name }}</span>
          <Check v-if="env.isActive" :size="13" class="text-accent-green flex-shrink-0" />
        </button>
      </MenuItem>
      <div class="my-1 border-t border-app-border" />
      <MenuItem v-slot="{ active }">
        <button
          class="w-full flex items-center gap-2 px-3 h-7 text-left text-xs"
          :class="active ? 'bg-app-hover' : ''"
          @click="onManage"
        >
          <Settings2 :size="13" class="text-app-muted" /> 管理环境
        </button>
      </MenuItem>
    </MenuItems>
  </Menu>
</template>

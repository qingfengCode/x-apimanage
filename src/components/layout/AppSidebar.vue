<script setup lang="ts">
import CollectionTree from "@/components/collection/CollectionTree.vue";
import HistoryPanel from "@/components/history/HistoryPanel.vue";
import MockPanel from "@/components/mock/MockPanel.vue";
import DocsPanel from "@/components/docs/DocsPanel.vue";
import EnvSelector from "@/components/environment/EnvSelector.vue";

type SideTab = "collections" | "history" | "mock" | "docs" | "runner";

defineProps<{
  activeTab: SideTab;
  onSwitch: (t: SideTab) => void;
  onManageEnv: () => void;
}>();
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- 环境选择器（导航已上移到左侧活动栏） -->
    <div class="px-1.5 pt-1.5 pb-1">
      <EnvSelector :on-manage="onManageEnv" />
    </div>

    <!-- 内容（runner 模式下侧栏仍显示集合树，便于浏览/右键运行） -->
    <div class="flex-1 min-h-0 border-t border-app-border/60">
      <CollectionTree v-show="activeTab === 'collections' || activeTab === 'runner'" />
      <MockPanel v-show="activeTab === 'mock'" />
      <HistoryPanel v-show="activeTab === 'history'" />
      <DocsPanel v-show="activeTab === 'docs'" />
    </div>
  </div>
</template>

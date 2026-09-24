<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { FolderTree, Power, Clock, FileText, Keyboard, PlayCircle } from "lucide-vue-next";
import AppSidebar from "./AppSidebar.vue";
import AppMain from "./AppMain.vue";
import TitleBar from "./TitleBar.vue";
import EnvManager from "@/components/environment/EnvManager.vue";
import ShortcutHelpModal from "@/components/common/ShortcutHelpModal.vue";
import CommandPalette from "@/components/common/CommandPalette.vue";
import { useWorkspaceStore } from "@/stores/workspace";

type SideTab = "collections" | "history" | "mock" | "docs" | "runner";

const RAIL_TABS: { key: SideTab; label: string; icon: any }[] = [
  { key: "collections", label: "集合", icon: FolderTree },
  { key: "mock", label: "Mock 服务", icon: Power },
  { key: "history", label: "历史", icon: Clock },
  { key: "docs", label: "文档", icon: FileText },
  { key: "runner", label: "集合运行 Runner", icon: PlayCircle },
];

const workspaceStore = useWorkspaceStore();
const sideTab = ref<SideTab>("collections");
const sidebarWidth = ref(260);
const showEnvManager = ref(false);
const showShortcuts = ref(false);
const showPalette = ref(false);

// Ctrl+P 命令面板
function onShortcutPalette() {
  showPalette.value = !showPalette.value;
}
onMounted(() => {
  window.addEventListener("shortcut:palette", onShortcutPalette);
  window.addEventListener("open:mock-panel", onOpenMockPanel);
  window.addEventListener("open:env-manager", onOpenEnvManager);
});
onBeforeUnmount(() => {
  window.removeEventListener("shortcut:palette", onShortcutPalette);
  window.removeEventListener("open:mock-panel", onOpenMockPanel);
  window.removeEventListener("open:env-manager", onOpenEnvManager);
});

/** 活动栏切换：runner 占据主区，其余（含 mock/docs）保持请求/文档/Mock Tab 视图 */
function switchTab(t: SideTab) {
  sideTab.value = t;
  workspaceStore.setMode(t === "runner" ? "runner" : "requests");
}

// 命令面板「Mock 服务」入口：切左侧栏到 Mock 面板（主区仍是 Tab）
function onOpenMockPanel() {
  switchTab("mock");
}

// 未定义变量警告等处的「环境管理」入口
function onOpenEnvManager() {
  showEnvManager.value = true;
}

// 监听 Ctrl+/（快捷键帮助）事件
function onShortcutHelp() {
  showShortcuts.value = true;
}
onMounted(() => window.addEventListener("shortcut:help", onShortcutHelp));
onBeforeUnmount(() => window.removeEventListener("shortcut:help", onShortcutHelp));

// 可拖拽分隔
const dragging = ref(false);
function onDragStart(e: MouseEvent) {
  e.preventDefault();
  dragging.value = true;
  window.addEventListener("mousemove", onDragMove);
  window.addEventListener("mouseup", onDragEnd);
}
function onDragMove(e: MouseEvent) {
  const w = Math.min(Math.max(e.clientX - 44, 180), 460);
  sidebarWidth.value = w;
}
function onDragEnd() {
  dragging.value = false;
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
}
onBeforeUnmount(() => {
  window.removeEventListener("mousemove", onDragMove);
  window.removeEventListener("mouseup", onDragEnd);
});
</script>

<template>
  <div class="h-full flex flex-col bg-app-bg text-app-text select-none">
    <!-- 自定义标题栏（无边框窗口） -->
    <TitleBar />

    <div class="flex-1 flex min-h-0">
      <!-- 活动栏：窄图标导航（VS Code 风格），点击切换侧栏面板 -->
      <nav class="w-11 flex-shrink-0 bg-app-surface border-r border-app-border flex flex-col items-center py-1.5 gap-0.5">
        <button
          v-for="t in RAIL_TABS"
          :key="t.key"
          class="relative w-9 h-9 rounded-lg flex items-center justify-center transition-colors"
          :class="
            sideTab === t.key
              ? 'text-app-text bg-app-hover'
              : 'text-app-muted hover:text-app-text hover:bg-app-hover/60'
          "
          :title="t.label"
          @click="sideTab = t.key"
        >
          <component :is="t.icon" :size="17" :stroke-width="sideTab === t.key ? 2.2 : 1.8" />
          <span v-if="sideTab === t.key" class="rail-indicator" />
        </button>

        <!-- 底部：快捷键帮助 -->
        <button
          class="mt-auto w-9 h-9 rounded-lg flex items-center justify-center text-app-muted hover:text-app-text hover:bg-app-hover/60 transition-colors"
          title="快捷键帮助 (Ctrl+/)"
          @click="showShortcuts = true"
        >
          <Keyboard :size="17" :stroke-width="1.8" />
        </button>
      </nav>

      <!-- 侧栏面板（最深 chrome 层） -->
      <div
        class="flex-shrink-0 bg-app-surface border-r border-app-border flex flex-col"
        :style="{ width: sidebarWidth + 'px' }"
      >
        <AppSidebar
          :active-tab="sideTab"
          :on-switch="switchTab"
          :on-manage-env="() => (showEnvManager = true)"
        />
      </div>

      <!-- 分隔条：4px 命中区，默认透明只露出边框线，悬停/拖拽时高亮 -->
      <div
        class="w-1 -ml-px z-10 flex-shrink-0 bg-transparent hover:bg-accent-blue/60 cursor-col-resize transition-colors"
        :class="{ 'bg-accent-blue/60': dragging }"
        @mousedown="onDragStart"
      />

      <!-- 主区 -->
      <div class="flex-1 min-w-0">
        <AppMain />
      </div>
    </div>

    <!-- 环境管理弹窗 -->
    <EnvManager v-model:show="showEnvManager" />
    <!-- 快捷键帮助 -->
    <ShortcutHelpModal v-model:show="showShortcuts" />
    <!-- 命令面板 Ctrl+P -->
    <CommandPalette v-model:show="showPalette" />
  </div>
</template>

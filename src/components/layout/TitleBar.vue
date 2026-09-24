<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref } from "vue";
import { getCurrentWindow, type Window as TauriWindow } from "@tauri-apps/api/window";
import { Minus, Square, Copy, X, Zap, RefreshCw } from "lucide-vue-next";
import { useUpdateStore } from "@/stores/update";
import UpdateDialog from "@/components/common/UpdateDialog.vue";

// 纯浏览器环境（无 Tauri internals）下 getCurrentWindow 会同步抛错，做兜底
function safeCurrentWindow(): TauriWindow | null {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
}
const win = safeCurrentWindow();

const updater = useUpdateStore();
const showUpdateDialog = ref(false);

/** 是否存在可用新版本（更新按钮上的红点；含下载中/已下载态） */
const hasUpdate = computed(() =>
  ["update-available", "downloading", "downloaded"].includes(updater.status)
);

const isMaximized = ref(false);
let unlisten: (() => void) | null = null;

// 跟踪最大化状态（切换 还原/最大化 图标）；启动时静默检查更新（尊重"跳过此版本"）
onMounted(async () => {
  if (win) {
    try {
      isMaximized.value = await win.isMaximized();
      unlisten = await win.onResized(async () => {
        try {
          isMaximized.value = await win.isMaximized();
        } catch {
          /* ignore */
        }
      });
    } catch {
      /* 非 Tauri 环境（纯浏览器调试）忽略 */
    }
  }
  // 静默检查：失败（未配置源/离线）不打扰，仅在有新版且未被跳过时亮红点
  updater.check(false).catch(() => {});
});
onBeforeUnmount(() => unlisten?.());

function minimize() {
  win?.minimize().catch(() => {});
}
function toggleMaximize() {
  win?.toggleMaximize().catch(() => {});
}
function close() {
  win?.close().catch(() => {});
}
</script>

<template>
  <div
    class="h-8 flex-shrink-0 flex items-stretch bg-app-surface border-b border-app-border select-none"
  >
    <!-- 品牌区 + 拖拽区 -->
    <div
      class="flex-1 min-w-0 flex items-center gap-2 pl-3"
      data-tauri-drag-region
      title="拖动移动窗口 / 双击最大化"
    >
      <div
        class="w-4 h-4 rounded bg-gradient-to-br from-[rgb(255,125,75)] to-[rgb(255,108,55)] flex items-center justify-center flex-shrink-0 shadow-[0_0_8px_rgb(var(--accent-orange)/0.4)]"
        data-tauri-drag-region
      >
        <Zap :size="9" class="text-white" stroke-width="2.5" data-tauri-drag-region />
      </div>
      <span
        class="text-xs font-semibold tracking-tight text-app-text/90"
        data-tauri-drag-region
      >
        x-apimanage
      </span>
    </div>

    <!-- 窗口控制按钮（Windows 风格，右侧） -->
    <div class="flex items-stretch flex-shrink-0">
      <!-- 更新检测：有新版本时红点提示 -->
      <button
        class="relative w-11 flex items-center justify-center text-app-muted hover:bg-app-hover hover:text-app-text transition-colors"
        :class="{ 'text-accent-orange': hasUpdate }"
        :title="hasUpdate ? '发现新版本，点击查看' : '检查更新'"
        @click="showUpdateDialog = true"
      >
        <RefreshCw :size="13" :class="{ 'animate-[spin_3s_linear_infinite]': hasUpdate }" />
        <span
          v-if="hasUpdate"
          class="absolute top-1.5 right-2 w-1.5 h-1.5 rounded-full bg-accent-orange shadow-[0_0_4px_rgb(var(--accent-orange)/0.8)]"
        />
      </button>
      <button
        class="w-11 flex items-center justify-center text-app-muted hover:bg-app-hover hover:text-app-text transition-colors"
        title="最小化"
        @click="minimize"
      >
        <Minus :size="14" />
      </button>
      <button
        class="w-11 flex items-center justify-center text-app-muted hover:bg-app-hover hover:text-app-text transition-colors"
        :title="isMaximized ? '还原' : '最大化'"
        @click="toggleMaximize"
      >
        <Copy v-if="isMaximized" :size="12" />
        <Square v-else :size="11" />
      </button>
      <button
        class="w-11 flex items-center justify-center text-app-muted hover:bg-accent-red hover:text-white transition-colors"
        title="关闭"
        @click="close"
      >
        <X :size="15" />
      </button>
    </div>
  </div>

  <!-- 更新弹窗 -->
  <UpdateDialog v-model:show="showUpdateDialog" />
</template>

<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from "vue";
import { Webhook, Terminal } from "lucide-vue-next";
import { useTabStore } from "@/stores/tab";
import { useWorkspaceStore } from "@/stores/workspace";
import TabBar from "@/components/common/TabBar.vue";
import RequestPanel from "@/components/request/RequestPanel.vue";
import EmptyState from "@/components/common/EmptyState.vue";
import AiAssistant from "@/components/ai/AiAssistant.vue";
import McpPanel from "@/components/ai/McpPanel.vue";
import CurlImportModal from "@/components/common/CurlImportModal.vue";
import DocTabView from "@/components/docs/DocTabView.vue";
import MockTabView from "@/components/mock/MockTabView.vue";
import RunnerWorkspace from "@/components/runner/RunnerWorkspace.vue";

const tabStore = useTabStore();
const workspaceStore = useWorkspaceStore();
const showCurlImport = ref(false);

// TabBar 的导入按钮通过事件触发（TabBar 不持有模态，保持展示层职责单一）
function onOpenCurlImport() {
  showCurlImport.value = true;
}

// 命令面板等处的「导入 cURL」入口（window 事件解耦）
function onGlobalCurlImport() {
  showCurlImport.value = true;
}
onMounted(() => window.addEventListener("open:curl-import", onGlobalCurlImport));
onBeforeUnmount(() => window.removeEventListener("open:curl-import", onGlobalCurlImport));
</script>

<template>
  <div class="flex flex-col h-full bg-app-bg">
    <div class="flex-1 min-h-0 flex">
      <div class="flex-1 min-w-0">
        <!-- 请求/文档/Mock Tab 工作区：v-show 保留 Monaco/表单状态，切到 runner 再回来不丢 -->
        <div v-show="workspaceStore.mode !== 'runner'" class="h-full flex flex-col">
          <TabBar @import-curl="onOpenCurlImport" />
          <div class="flex-1 min-h-0">
            <!-- 激活 Tab 为 Mock 路由时显示路由编辑视图 -->
            <MockTabView v-if="tabStore.activeMockTab" />
            <!-- 激活 Tab 为文档时显示文档视图（编辑状态存在 Tab 上，切换 Tab 重建组件不丢） -->
            <DocTabView v-else-if="tabStore.activeDocTab" />
            <RequestPanel
              v-else-if="tabStore.activeDraft"
              :key="tabStore.activeTabId ?? 'none'"
            />
            <EmptyState
              v-else
              :icon="Webhook"
              title="开始你的第一个请求"
              hint="点击上方 + 新建请求 (Ctrl+T)，或从左侧集合中选择"
            >
              <template #actions>
                <button
                  class="btn-ghost border border-app-border hover:border-accent-orange/60 mt-1"
                  @click="showCurlImport = true"
                >
                  <Terminal :size="13" /> 导入 cURL 命令
                </button>
              </template>
            </EmptyState>
          </div>
        </div>

        <!-- 集合运行工作区 -->
        <RunnerWorkspace v-show="workspaceStore.mode === 'runner'" />
      </div>
      <!-- AI 助手 / MCP 服务右侧抽屉 -->
      <AiAssistant />
      <McpPanel />
    </div>

    <CurlImportModal v-model:show="showCurlImport" />
  </div>
</template>

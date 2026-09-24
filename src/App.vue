<script setup lang="ts">
import { onBeforeUnmount, onMounted, watch } from "vue";
import AppLayout from "@/components/layout/AppLayout.vue";
import ToastHost from "@/components/common/ToastHost.vue";
import { useCollectionStore } from "@/stores/collection";
import { useEnvironmentStore } from "@/stores/environment";
import { useHistoryStore } from "@/stores/history";
import { useMockStore } from "@/stores/mock";
import { useDocsStore } from "@/stores/docs";
import { useAiStore } from "@/stores/ai";
import { useTabStore } from "@/stores/tab";
import { useGlobalShortcuts } from "@/composables/useGlobalShortcuts";
import { useToast } from "@/composables/useToast";

const collectionStore = useCollectionStore();
const envStore = useEnvironmentStore();
const historyStore = useHistoryStore();
const mockStore = useMockStore();
const docsStore = useDocsStore();
const aiStore = useAiStore();
const tabStore = useTabStore();
const toast = useToast();

// 注册全局快捷键
useGlobalShortcuts();

// 窗口关闭前同步落盘未保存草稿与会话快照（防抖定时器可能来不及触发）
window.addEventListener("beforeunload", () => {
  tabStore.flushAutosave();
  tabStore.persistSession();
});

onMounted(async () => {
  // 先恢复上次退出前未保存的请求草稿
  try {
    const n = tabStore.initDrafts();
    if (n) toast.push(`已恢复 ${n} 个未保存的请求草稿`, "info", 4000);
  } catch {
    /* 恢复失败不阻断 */
  }
  const tasks: [string, () => Promise<void>][] = [
    ["集合", () => collectionStore.init()],
    ["环境", () => envStore.init()],
    ["历史", () => historyStore.init()],
    ["Mock", () => mockStore.init()],
    ["文档", () => docsStore.init()],
    ["AI", () => aiStore.loadSettings().then(() => aiStore.loadSessions())],
  ];
  for (const [name, fn] of tasks) {
    try {
      await fn();
    } catch {
      toast.push(`${name}数据加载失败`, "error", 8000, {
        label: "重试",
        onClick: () => {
          fn().catch(() =>
            toast.push(`${name}数据加载失败`, "error", 8000, { label: "重试", onClick: () => fn() })
          );
        },
      });
    }
  }

  // 数据齐了再恢复上次会话的 Tab（请求/文档/Mock 按各自 store 找回）
  try {
    const n = tabStore.restoreSession({
      findRequest: (id) => {
        for (const colId of Object.keys(collectionStore.requests)) {
          const r = collectionStore.requests[colId].find((x) => x.id === id);
          if (r) return r;
        }
        return undefined;
      },
      findDoc: (id) => docsStore.documents.find((d) => d.id === id),
      findMock: (id) => (mockStore.routes.some((r) => r.id === id) ? { id } : undefined),
    });
    if (n) toast.push(`已恢复上次会话的 ${n} 个标签页`, "info", 3000);
  } catch {
    /* 恢复失败不阻断 */
  }
});

// 会话持久化：Tab 列表/激活态变化后防抖落盘（关闭前再兜底一次）
let sessionTimer: ReturnType<typeof setTimeout> | undefined;
const stopSession = watch(
  () => [tabStore.tabs.map((t) => t.id).join("|"), tabStore.activeTabId] as const,
  () => {
    if (sessionTimer) clearTimeout(sessionTimer);
    sessionTimer = setTimeout(() => tabStore.persistSession(), 500);
  }
);
onBeforeUnmount(() => {
  stopSession();
  if (sessionTimer) clearTimeout(sessionTimer);
});
</script>

<template>
  <AppLayout />
  <ToastHost />
</template>

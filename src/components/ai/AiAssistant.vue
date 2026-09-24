<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref, watch } from "vue";
import { marked } from "marked";
import {
  Bot,
  Send,
  Settings,
  Trash2,
  Wrench,
  Loader2,
  CircleAlert,
  User,
  Paperclip,
  Square,
  History,
  MessageSquare,
  Plus,
} from "lucide-vue-next";
import { useAiStore } from "@/stores/ai";
import { useTabStore } from "@/stores/tab";
import { useDocsStore } from "@/stores/docs";
import { useCollectionStore } from "@/stores/collection";
import { makeCodeCopyHandler } from "@/composables/markdownClick";
import AiSettingsModal from "./AiSettingsModal.vue";

const aiStore = useAiStore();
const tabStore = useTabStore();
const docsStore = useDocsStore();
const collectionStore = useCollectionStore();

const input = ref("");
const listRef = ref<HTMLElement | null>(null);
const showSettings = ref(false);
const onListClick = makeCodeCopyHandler(listRef);

// ---- 面板宽度拖拽（左边缘） ----
const draggingWidth = ref(false);
function onWidthDragStart(e: MouseEvent) {
  e.preventDefault();
  draggingWidth.value = true;
  const onMove = (ev: MouseEvent) => {
    aiStore.setPanelWidth(window.innerWidth - ev.clientX);
  };
  const onUp = () => {
    draggingWidth.value = false;
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  };
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

/** 当前请求上下文（发给 AI 供其理解正在调试的接口） */
const currentContext = computed<string | null>(() => {
  const d = tabStore.activeDraft;
  if (!d) return null;
  const lines = [
    `请求名称: ${d.name}`,
    `方法: ${d.method}`,
    `URL: ${d.url || "(空)"}`,
  ];
  const params = d.params.filter((p) => p.key);
  if (params.length) lines.push(`参数: ${params.map((p) => `${p.key}=${p.value}`).join("&")}`);
  const headers = d.headers.filter((h) => h.key);
  if (headers.length) lines.push(`请求头: ${headers.map((h) => `${h.key}: ${h.value}`).join("; ")}`);
  if (d.body) {
    if (d.body.mode === "raw") lines.push(`Body (${d.body.mimeType}): ${d.body.raw.slice(0, 500)}`);
    else lines.push(`Body (${d.body.mode}): ${d.body.items.map((i) => `${i.key}=${i.value}`).join("&")}`);
  }
  return lines.join("\n");
});

function renderMd(text: string): string {
  return marked.parse(text, { async: false }) as string;
}

async function submit() {
  const text = input.value;
  if (!text.trim() || aiStore.busy) return;
  input.value = "";
  await aiStore.send(text, currentContext.value);
  // AI 可能创建了请求/文档，刷新左侧列表
  collectionStore.init().catch(() => {});
  docsStore.init().catch(() => {});
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey && !e.isComposing) {
    e.preventDefault();
    submit();
  }
}

// 新消息 / 流式增量自动滚动到底部；用户向上回看历史（离开底部）时不再强拖
const userScrolledUp = ref(false);
const SCROLL_STICKY_GAP = 140;
function onListScroll() {
  const el = listRef.value;
  if (!el) return;
  userScrolledUp.value = el.scrollHeight - el.scrollTop - el.clientHeight > SCROLL_STICKY_GAP;
}
watch(
  () => [aiStore.items.length, aiStore.status, lastTextLen()] as const,
  async () => {
    await nextTick();
    const el = listRef.value;
    if (!el || userScrolledUp.value) return;
    el.scrollTo({ top: el.scrollHeight });
  }
);
function lastTextLen(): number {
  const last = aiStore.items[aiStore.items.length - 1];
  return last ? last.text.length : 0;
}

// ---- 会话下拉 ----
const showSessions = ref(false);
function relativeTime(ts: number): string {
  const diff = Date.now() - ts;
  if (diff < 60_000) return "刚刚";
  if (diff < 3600_000) return `${Math.floor(diff / 60_000)} 分钟前`;
  if (diff < 86400_000) return `${Math.floor(diff / 3600_000)} 小时前`;
  return `${Math.floor(diff / 86400_000)} 天前`;
}
async function pickSession(id: string) {
  showSessions.value = false;
  await aiStore.selectSession(id);
}
function onClickOutside(e: MouseEvent) {
  const el = e.target as HTMLElement;
  if (!el.closest("[data-sessions-popover]") && !el.closest("[data-sessions-toggle]")) {
    showSessions.value = false;
  }
}
watch(showSessions, (v) => {
  window.removeEventListener("click", onClickOutside);
  if (v) window.addEventListener("click", onClickOutside);
});
// 面板整体卸载（Ctrl+I 收起）时清掉窗口级监听，避免累积泄漏
onBeforeUnmount(() => {
  window.removeEventListener("click", onClickOutside);
});

const QUICK_PROMPTS = [
  { label: "为当前接口生成 API 文档", text: "请根据当前请求上下文，实际发送请求验证后，为这个接口编写一份完整的 API 文档（含参数表、请求/响应示例）。" },
  { label: "创建调试配置", text: "帮我创建一个 API 调试配置：" },
  { label: "写产品文档", text: "帮我写一份产品文档：" },
];

</script>

<template>
  <aside
    v-if="aiStore.panelVisible"
    class="flex-shrink-0 border-l border-app-border bg-app-surface flex flex-col min-h-0 relative"
    :style="{ width: aiStore.panelWidth + 'px' }"
  >
    <!-- 左边缘宽度拖拽条 -->
    <div
      class="w-1 -ml-px z-10 absolute left-0 top-0 bottom-0 bg-transparent hover:bg-accent-purple/60 cursor-col-resize transition-colors"
      :class="{ 'bg-accent-purple/60': draggingWidth }"
      @mousedown="onWidthDragStart"
    />

    <!-- 头部 -->
    <div class="flex items-center gap-2 px-3 py-2 border-b border-app-border">
      <div class="w-6 h-6 rounded-md bg-gradient-to-br from-[rgb(188,140,255)] to-[rgb(120,100,255)] flex items-center justify-center flex-shrink-0">
        <Bot :size="13" class="text-white" />
      </div>
      <span class="text-sm font-medium">AI 助手</span>
      <span
        v-if="!aiStore.settings.model"
        class="text-[10px] px-1.5 py-0.5 rounded bg-accent-orange/15 text-accent-orange"
      >
        未配置
      </span>
      <div class="ml-auto flex items-center gap-0.5">
        <!-- 会话历史 -->
        <div class="relative">
          <button
            class="icon-btn"
            :class="{ 'text-accent-purple': showSessions }"
            title="历史会话"
            data-sessions-toggle
            @click="showSessions = !showSessions"
          >
            <History :size="14" />
          </button>
          <div
            v-if="showSessions"
            data-sessions-popover
            class="absolute right-0 top-full mt-1.5 z-40 w-64 bg-app-surface2 border border-app-border rounded-md shadow-2xl menu-enter py-1"
          >
            <button
              class="w-full flex items-center gap-2 px-3 py-1.5 text-xs text-accent-purple hover:bg-app-hover"
              @click="aiStore.newSession(); showSessions = false"
            >
              <Plus :size="13" /> 新建会话
            </button>
            <div v-if="aiStore.sessions.length" class="my-1 border-t border-app-border" />
            <div class="max-h-72 overflow-auto">
              <div
                v-for="s in aiStore.sessions"
                :key="s.id"
                class="group flex items-center gap-2 px-3 py-1.5 cursor-pointer hover:bg-app-hover"
                :class="{ 'bg-app-hover': s.id === aiStore.currentSessionId }"
                @click="pickSession(s.id)"
              >
                <MessageSquare :size="12" class="text-app-muted flex-shrink-0" />
                <div class="flex-1 min-w-0">
                  <div class="text-xs truncate">{{ s.title }}</div>
                  <div class="text-[10px] text-app-muted">{{ relativeTime(s.updatedAt) }}</div>
                </div>
                <button
                  class="icon-btn opacity-0 group-hover:opacity-100"
                  title="删除会话"
                  @click.stop="aiStore.deleteSession(s.id)"
                >
                  <Trash2 :size="11" />
                </button>
              </div>
            </div>
          </div>
        </div>
        <button class="icon-btn" title="清空当前会话" @click="aiStore.clearChat()">
          <Trash2 :size="14" />
        </button>
        <button class="icon-btn" title="AI / MCP 设置" @click="showSettings = true">
          <Settings :size="14" />
        </button>
      </div>
    </div>

    <!-- 消息列表 -->
    <div
      ref="listRef"
      class="flex-1 min-h-0 overflow-auto px-3 py-3 flex flex-col gap-3"
      @scroll.passive="onListScroll"
    >
      <!-- 欢迎页 -->
      <div v-if="!aiStore.items.length" class="flex flex-col gap-3 pt-4">
        <div class="flex flex-col items-center gap-2 text-center py-4">
          <div class="w-12 h-12 rounded-full bg-app-bg border border-app-border flex items-center justify-center">
            <Bot :size="22" class="text-accent-purple" />
          </div>
          <div class="text-sm text-app-text/80">我能帮你做什么？</div>
          <div class="text-xs text-app-muted leading-relaxed max-w-[260px]">
            编写 API 调试配置并实际发送验证、生成 API 接口文档、撰写产品文档
          </div>
        </div>
        <button
          v-for="q in QUICK_PROMPTS"
          :key="q.label"
          class="text-left text-xs px-3 py-2 rounded-md bg-app-bg border border-app-border hover:border-accent-purple/60 transition-colors text-app-muted hover:text-app-text"
          @click="input = q.text"
        >
          {{ q.label }}
        </button>
      </div>

      <template v-for="(m, i) in aiStore.items" :key="i">
        <!-- 用户 -->
        <div v-if="m.kind === 'user'" class="flex gap-2 justify-end">
          <div class="max-w-[85%] rounded-lg rounded-tr-sm bg-accent-orange/15 border border-accent-orange/25 px-3 py-2 text-[13px] text-app-text whitespace-pre-wrap break-words">
            {{ m.text }}
          </div>
          <div class="w-6 h-6 rounded-md bg-app-surface2 border border-app-border flex items-center justify-center flex-shrink-0">
            <User :size="12" class="text-app-muted" />
          </div>
        </div>

        <!-- 助手：流式中显示纯文本+光标，完成后渲染 Markdown -->
        <div v-else-if="m.kind === 'assistant'" class="flex gap-2">
          <div class="w-6 h-6 rounded-md bg-gradient-to-br from-[rgb(188,140,255)] to-[rgb(120,100,255)] flex items-center justify-center flex-shrink-0">
            <Bot :size="12" class="text-white" />
          </div>
          <div
            v-if="m.streaming"
            class="max-w-[85%] rounded-lg rounded-tl-md bg-app-bg border border-app-border px-3 py-2 text-[13px] text-app-text whitespace-pre-wrap break-words font-sans"
          >{{ m.text }}<span class="inline-block w-[7px] h-[14px] ml-0.5 align-middle bg-accent-purple animate-pulse rounded-sm" /></div>
          <div
            v-else
            class="ai-md max-w-[85%] rounded-lg rounded-tl-md bg-app-bg border border-app-border px-3 py-2 text-[13px]"
            v-html="renderMd(m.text)"
          />
        </div>

        <!-- 工具执行 -->
        <div v-else-if="m.kind === 'tool'" class="flex gap-2 items-start pl-8">
          <div
            class="flex items-center gap-1.5 text-[11px] px-2 py-1 rounded-md border font-mono max-w-full"
            :class="m.ok
              ? 'bg-accent-green/10 border-accent-green/25 text-accent-green'
              : 'bg-accent-red/10 border-accent-red/25 text-accent-red'"
            :title="m.text"
          >
            <Wrench :size="11" class="flex-shrink-0" />
            <span class="truncate">{{ m.toolName }} {{ m.ok ? "✓" : "✗" }}</span>
          </div>
        </div>

        <!-- 错误 -->
        <div v-else-if="m.kind === 'error'" class="flex gap-2 items-start pl-8">
          <div class="flex items-start gap-1.5 text-[11px] px-2.5 py-1.5 rounded-md bg-accent-red/10 border border-accent-red/25 text-accent-red max-w-full">
            <CircleAlert :size="12" class="flex-shrink-0 mt-0.5" />
            <span class="break-words">{{ m.text }}</span>
          </div>
        </div>

        <!-- 状态（已停止等） -->
        <div v-else-if="m.kind === 'status'" class="pl-8 text-[11px] text-app-muted italic">
          {{ m.text }}
        </div>
      </template>

      <!-- 思考中 -->
      <div v-if="aiStore.busy" class="flex gap-2 items-center pl-8 text-xs text-app-muted">
        <Loader2 :size="13" class="animate-spin" />
        <span>{{ aiStore.status || "处理中…" }}</span>
      </div>
    </div>

    <!-- 输入区 -->
    <div class="border-t border-app-border p-2.5 flex flex-col gap-2">
      <label class="flex items-center gap-1.5 text-[11px] text-app-muted cursor-pointer select-none">
        <input
          type="checkbox"
          v-model="aiStore.attachContext"
          class="accent-accent-orange"
          :disabled="!currentContext"
        />
        <Paperclip :size="11" />
        附带当前请求上下文
      </label>
      <div
        class="flex items-end gap-1.5 rounded-lg bg-app-bg border border-app-border focus-within:border-accent-purple/60 transition-colors p-1.5"
      >
        <textarea
          v-model="input"
          rows="2"
          placeholder="描述你的需求，例如：给 https://api.github.com/users/octocat 创建调试配置并发送验证…"
          class="flex-1 bg-transparent text-[13px] outline-none resize-none max-h-32 px-1"
          :disabled="aiStore.busy"
          @keydown="onKeydown"
        />
        <!-- 发送 / 停止 -->
        <button
          v-if="aiStore.busy"
          class="h-8 w-8 rounded-md bg-app-hover hover:brightness-110 text-app-text flex items-center justify-center flex-shrink-0 transition-[filter]"
          title="停止生成"
          @click="aiStore.stop()"
        >
          <Square :size="13" />
        </button>
        <button
          v-else
          class="h-8 w-8 rounded-md bg-accent-purple hover:brightness-110 text-white flex items-center justify-center disabled:opacity-50 flex-shrink-0 transition-[filter]"
          :disabled="!input.trim()"
          title="发送 (Enter)"
          @click="submit"
        >
          <Send :size="14" />
        </button>
      </div>
    </div>

    <AiSettingsModal v-model:show="showSettings" />
  </aside>
</template>

<style scoped>
/* AI 回复的 Markdown 排版 */
.ai-md :deep(p) { margin: 0.35em 0; }
.ai-md :deep(p:first-child) { margin-top: 0; }
.ai-md :deep(p:last-child) { margin-bottom: 0; }
.ai-md :deep(h1),
.ai-md :deep(h2),
.ai-md :deep(h3),
.ai-md :deep(h4) {
  font-weight: 600;
  margin: 0.8em 0 0.4em;
  color: rgb(var(--app-text));
}
.ai-md :deep(h1) { font-size: 1.15em; }
.ai-md :deep(h2) { font-size: 1.08em; }
.ai-md :deep(h3),
.ai-md :deep(h4) { font-size: 1em; }
.ai-md :deep(ul),
.ai-md :deep(ol) { padding-left: 1.4em; margin: 0.35em 0; }
.ai-md :deep(li) { margin: 0.15em 0; }
.ai-md :deep(code) {
  font-family: 'JetBrains Mono', Menlo, Consolas, monospace;
  font-size: 0.9em;
  background: rgb(var(--app-surface2));
  padding: 0.1em 0.35em;
  border-radius: 3px;
}
.ai-md :deep(pre) {
  background: rgb(var(--app-surface2));
  border: 1px solid rgb(var(--app-border) / 0.6);
  border-radius: 6px;
  padding: 0.6em 0.8em;
  overflow-x: auto;
  margin: 0.5em 0;
}
.ai-md :deep(pre code) { background: transparent; padding: 0; }
.ai-md :deep(pre) { cursor: copy; }
.ai-md :deep(pre:hover) { border-color: rgb(var(--accent-blue) / 0.5); }
.ai-md :deep(table) { border-collapse: collapse; margin: 0.5em 0; width: 100%; font-size: 0.92em; }
.ai-md :deep(th),
.ai-md :deep(td) {
  border: 1px solid rgb(var(--app-border));
  padding: 0.3em 0.6em;
  text-align: left;
}
.ai-md :deep(th) { background: rgb(var(--app-surface2)); font-weight: 600; }
.ai-md :deep(blockquote) {
  border-left: 3px solid rgb(var(--app-border));
  padding-left: 0.8em;
  color: rgb(var(--app-muted));
  margin: 0.5em 0;
}
.ai-md :deep(a) { color: rgb(var(--accent-blue)); text-decoration: underline; }
.ai-md :deep(hr) { border: none; border-top: 1px solid rgb(var(--app-border)); margin: 0.8em 0; }
</style>

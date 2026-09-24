<script setup lang="ts">
import { ref, watch } from "vue";
import {
  Plug,
  Play,
  Square,
  Copy,
  Check,
  KeyRound,
  Loader2,
  ShieldCheck,
  RefreshCw,
  ShieldOff,
} from "lucide-vue-next";
import { useAiStore } from "@/stores/ai";
import { copyText } from "@/utils/curl";
import { useToast } from "@/composables/useToast";

const aiStore = useAiStore();
const toast = useToast();

const busy = ref(false);
const tokenBusy = ref(false);
const copiedJson = ref(false);
const copiedToken = ref(false);
const showToken = ref(false);

watch(
  () => aiStore.mcpPanelVisible,
  (v) => {
    if (v) aiStore.refreshMcpInfo();
  }
);

const info = () => aiStore.mcpInfo;

async function toggleServer() {
  if (busy.value) return;
  busy.value = true;
  try {
    const turningOn = !info()?.running;
    const url = await aiStore.setMcp(turningOn);
    toast.push(url ? `MCP 服务已启动：${url}` : "MCP 服务已停止", "success", 3000);
    await aiStore.refreshMcpInfo();
  } catch (e) {
    toast.push(`MCP 启动失败：${String(e)}`, "error", 5000);
  } finally {
    busy.value = false;
  }
}

async function onPortChange(e: Event) {
  const v = Number((e.target as HTMLInputElement).value);
  if (v >= 1024 && v <= 65535 && v !== aiStore.settings.mcpPort) {
    try {
      await aiStore.changeMcpPort(v);
      if (aiStore.mcpUrl) toast.push(`MCP 已切换到端口 ${v}`, "success", 2500);
      await aiStore.refreshMcpInfo();
    } catch (err) {
      toast.push(String(err), "error", 5000);
    }
  }
}

async function copyJson() {
  if (await copyText(aiStore.mcpConfigJson())) {
    copiedJson.value = true;
    toast.push("已复制，粘贴到 Claude Code / Cursor 等 MCP 客户端配置即可", "success", 2500);
    setTimeout(() => (copiedJson.value = false), 1500);
  }
}

async function copyToken() {
  if (info()?.token && (await copyText(info()!.token))) {
    copiedToken.value = true;
    setTimeout(() => (copiedToken.value = false), 1500);
  }
}

async function genToken() {
  if (tokenBusy.value) return;
  tokenBusy.value = true;
  try {
    await aiStore.generateToken();
    toast.push("已生成新密钥（旧密钥立即失效，服务已切换为局域网 + 鉴权）", "success", 3500);
  } catch (e) {
    toast.push(String(e), "error", 5000);
  } finally {
    tokenBusy.value = false;
  }
}

async function revoke() {
  if (!window.confirm("吊销访问密钥？MCP 将恢复为仅本机访问、无需鉴权。")) return;
  tokenBusy.value = true;
  try {
    await aiStore.revokeToken();
    toast.push("已吊销密钥，MCP 仅本机可访问", "success", 3000);
  } catch (e) {
    toast.push(String(e), "error", 5000);
  } finally {
    tokenBusy.value = false;
  }
}
</script>

<template>
  <aside
    v-if="aiStore.mcpPanelVisible"
    class="w-[380px] flex-shrink-0 border-l border-app-border bg-app-surface flex flex-col min-h-0"
  >
    <!-- 头部 -->
    <div class="flex items-center gap-2 px-3 py-2 border-b border-app-border">
      <div class="w-6 h-6 rounded-md bg-gradient-to-br from-[rgb(61,138,255)] to-[rgb(80,110,245)] flex items-center justify-center flex-shrink-0">
        <Plug :size="13" class="text-white" />
      </div>
      <span class="text-sm font-medium">MCP 服务</span>
      <span
        class="flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded"
        :class="info()?.running
          ? 'bg-accent-green/15 text-accent-green'
          : 'bg-app-hover text-app-muted'"
      >
        <span v-if="info()?.running" class="w-1.5 h-1.5 rounded-full bg-accent-green animate-pulse" />
        {{ info()?.running ? "运行中" : "已停止" }}
      </span>
    </div>

    <div class="flex-1 min-h-0 overflow-auto p-3 flex flex-col gap-3">
      <!-- 服务控制 -->
      <section class="rounded-lg border border-app-border bg-app-bg p-3 flex flex-col gap-2.5">
        <div class="flex items-center gap-2">
          <span class="text-xs text-app-muted flex-shrink-0">服务状态</span>
          <span v-if="info()?.url" class="font-mono text-[11px] text-accent-green truncate" :title="info()!.url!">
            {{ info()!.url }}
          </span>
          <span v-else class="text-[11px] text-app-muted">未运行</span>
        </div>

        <!-- 启停独立一行 -->
        <div class="flex items-center gap-2">
          <label class="flex items-center gap-1 text-xs text-app-muted">
            端口
            <input
              type="number"
              min="1024"
              max="65535"
              :value="aiStore.settings.mcpPort"
              class="w-16 bg-app-surface2 border border-app-border rounded px-1 py-px text-[11px] font-mono outline-none focus:border-accent-blue"
              @change="onPortChange"
            />
          </label>
          <button
            v-if="info()?.running"
            class="ml-auto px-4 py-1.5 rounded-md text-xs bg-accent-red/10 text-accent-red hover:bg-accent-red/20 transition-colors flex items-center gap-1.5"
            :disabled="busy"
            @click="toggleServer"
          >
            <Loader2 v-if="busy" :size="12" class="animate-spin" />
            <Square v-else :size="12" /> 停止服务
          </button>
          <button
            v-else
            class="ml-auto px-4 py-1.5 rounded-md text-xs bg-accent-green/15 text-accent-green hover:bg-accent-green/25 transition-colors flex items-center gap-1.5"
            :disabled="busy"
            @click="toggleServer"
          >
            <Loader2 v-if="busy" :size="12" class="animate-spin" />
            <Play v-else :size="12" /> 启动服务
          </button>
        </div>

        <p class="text-[11px] text-app-muted/80 leading-relaxed">
          把本应用的调试配置 / 发送请求 / 写文档能力与集合数据，通过 MCP 协议暴露给 Claude Code、Cursor 等外部 AI 客户端。
        </p>
      </section>

      <!-- 访问密钥 -->
      <section class="rounded-lg border border-app-border bg-app-bg p-3 flex flex-col gap-2.5">
        <div class="flex items-center gap-1.5 text-xs font-semibold text-app-text">
          <KeyRound :size="13" class="text-accent-yellow" /> 访问密钥
          <span
            class="ml-auto flex items-center gap-1 text-[10px] font-normal"
            :class="info()?.token ? 'text-accent-green' : 'text-app-muted'"
          >
            <ShieldCheck v-if="info()?.token" :size="11" />
            {{ info()?.token ? "鉴权已启用 · 局域网可访问" : "未设置 · 仅本机可访问" }}
          </span>
        </div>

        <!-- 当前密钥 -->
        <div v-if="info()?.token" class="flex items-center gap-1.5">
          <code class="flex-1 min-w-0 truncate font-mono text-[11px] px-2 py-1.5 rounded bg-app-surface2 border border-app-border text-app-text">
            {{ showToken ? info()!.token : "•".repeat(32) }}
          </code>
          <button class="icon-btn flex-shrink-0 text-[11px]" :title="showToken ? '隐藏' : '显示'" @click="showToken = !showToken">
            {{ showToken ? "隐藏" : "显示" }}
          </button>
          <button class="icon-btn flex-shrink-0" :title="copiedToken ? '已复制' : '复制密钥'" @click="copyToken">
            <Check v-if="copiedToken" :size="12" class="text-accent-green" />
            <Copy v-else :size="12" />
          </button>
        </div>

        <div class="flex items-center gap-2">
          <button
            class="btn-ghost btn-sm"
            :disabled="tokenBusy"
            @click="genToken"
          >
            <Loader2 v-if="tokenBusy" :size="12" class="animate-spin" />
            <RefreshCw v-else :size="12" />
            {{ info()?.token ? "重新生成" : "生成密钥" }}
          </button>
          <button
            v-if="info()?.token"
            class="btn-danger btn-sm"
            :disabled="tokenBusy"
            @click="revoke"
          >
            <ShieldOff :size="12" /> 吊销
          </button>
        </div>
        <p class="text-[11px] text-app-muted/80 leading-relaxed">
          生成密钥后服务绑定 0.0.0.0，局域网设备凭密钥访问（回环地址同样需要）；吊销后恢复仅本机、免鉴权。
        </p>
      </section>

      <!-- 客户端配置 -->
      <section class="rounded-lg border border-app-border bg-app-bg p-3 flex flex-col gap-2">
        <div class="flex items-center gap-1.5 text-xs font-semibold text-app-text">
          客户端连接配置
          <button
            class="ml-auto btn-ghost btn-sm"
            :title="copiedJson ? '已复制' : '复制 JSON'"
            @click="copyJson"
          >
            <Check v-if="copiedJson" :size="12" class="text-accent-green" />
            <Copy v-else :size="12" /> 复制
          </button>
        </div>
        <pre class="font-mono text-[11px] leading-relaxed px-2.5 py-2 rounded bg-app-surface2 border border-app-border text-app-text overflow-x-auto select-text">{{ aiStore.mcpConfigJson() }}</pre>
        <p class="text-[11px] text-app-muted/80 leading-relaxed">
          复制后合并进 Claude Code / Cursor 等客户端的 MCP 配置文件即可连接{{ info()?.token ? "（已含访问密钥）" : "" }}。
        </p>
      </section>
    </div>
  </aside>
</template>

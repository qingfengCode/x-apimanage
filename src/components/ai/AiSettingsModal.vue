<script setup lang="ts">
import { ref, watch } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { Bot, Loader2, Eye, EyeOff } from "lucide-vue-next";
import { useAiStore } from "@/stores/ai";
import { useToast } from "@/composables/useToast";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const aiStore = useAiStore();
const toast = useToast();

/** 常用模型服务预设（点击填充，key 可后补） */
const PRESETS = [
  { label: "DeepSeek", baseUrl: "https://api.deepseek.com/v1", model: "deepseek-chat" },
  { label: "智谱 GLM", baseUrl: "https://open.bigmodel.cn/api/paas/v4", model: "glm-4.6" },
  { label: "Kimi", baseUrl: "https://api.moonshot.cn/v1", model: "moonshot-v1-8k" },
  { label: "OpenAI", baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini" },
  { label: "Ollama", baseUrl: "http://localhost:11434/v1", model: "qwen2.5:7b" },
];

function applyPreset(p: (typeof PRESETS)[number]) {
  form.value.baseUrl = p.baseUrl;
  form.value.model = p.model;
}

const showKey = ref(false);

const form = ref({
  baseUrl: "",
  apiKey: "",
  model: "",
  systemPrompt: "",
  // 以下字段不由本弹窗编辑，仅随表单一并保存（MCP 管理在顶部 MCP 面板）
  mcpEnabled: false,
  mcpPort: 8765,
  mcpToken: "",
});

const testing = ref(false);
const testResult = ref<string | null>(null);

watch(
  () => props.show,
  async (show) => {
    if (show) {
      if (!aiStore.settingsLoaded) await aiStore.loadSettings().catch(() => {});
      form.value = { ...aiStore.settings };
      testResult.value = null;
    }
  }
);

async function test() {
  testing.value = true;
  testResult.value = null;
  // 先保存当前表单（测试用的是已保存配置）
  try {
    await aiStore.saveSettings({ ...form.value });
    const reply = await aiStore.testConnection();
    testResult.value = `连接成功：${reply.slice(0, 80)}`;
  } catch (e) {
    testResult.value = `失败：${String(e)}`;
  } finally {
    testing.value = false;
  }
}

async function save() {
  await aiStore.saveSettings({ ...form.value });
  emit("update:show", false);
  toast.push("AI 设置已保存", "success");
}

function close() {
  emit("update:show", false);
}
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-50" @close="close">
      <TransitionChild
        as="template"
        enter="duration-200 ease-out" enter-from="opacity-0" enter-to="opacity-100"
        leave="duration-150 ease-in" leave-from="opacity-100" leave-to="opacity-0"
      >
        <div class="fixed inset-0 bg-black/50" />
      </TransitionChild>

      <div class="fixed inset-0 flex items-center justify-center p-4">
        <TransitionChild
          as="template"
          enter="duration-150 ease-out" enter-from="opacity-0 scale-95" enter-to="opacity-100 scale-100"
          leave="duration-100 ease-in" leave-from="opacity-100 scale-100" leave-to="opacity-0 scale-95"
        >
          <DialogPanel
            class="w-[520px] max-w-full bg-app-surface2 border border-app-border rounded-md shadow-2xl"
          >
            <DialogTitle class="px-4 py-3 border-b border-app-border text-sm font-medium flex items-center">
              AI 模型设置
              <button class="ml-auto btn-ghost btn-sm" @click="close">关闭</button>
            </DialogTitle>

            <div class="p-4 flex flex-col gap-3 max-h-[70vh] overflow-auto">
              <div class="flex items-center gap-1.5 text-xs font-semibold text-app-text">
                <Bot :size="13" class="text-accent-purple" /> 模型服务（OpenAI 兼容）
              </div>
              <!-- 快捷预设 -->
              <div class="flex items-center gap-1 flex-wrap">
                <span class="text-[11px] text-app-muted">快速填充：</span>
                <button
                  v-for="p in PRESETS"
                  :key="p.label"
                  class="text-[11px] px-2 py-0.5 rounded-md border transition-colors"
                  :class="
                    form.baseUrl === p.baseUrl
                      ? 'border-accent-purple/60 text-accent-purple bg-accent-purple/10'
                      : 'border-app-border text-app-muted hover:text-app-text hover:border-app-muted'
                  "
                  :title="`${p.baseUrl} · ${p.model}`"
                  @click="applyPreset(p)"
                >
                  {{ p.label }}
                </button>
              </div>
              <label class="text-xs text-app-muted">Base URL
                <input
                  v-model="form.baseUrl"
                  class="input w-full mt-1 text-xs font-mono"
                  placeholder="https://api.deepseek.com/v1"
                  spellcheck="false"
                />
              </label>
              <label class="text-xs text-app-muted">API Key
                <div class="relative mt-1">
                  <input
                    v-model="form.apiKey"
                    :type="showKey ? 'text' : 'password'"
                    class="input w-full text-xs font-mono pr-8"
                    placeholder="sk-..."
                    spellcheck="false"
                  />
                  <button
                    class="absolute right-1.5 top-1/2 -translate-y-1/2 text-app-muted hover:text-app-text"
                    :title="showKey ? '隐藏' : '显示'"
                    type="button"
                    @click="showKey = !showKey"
                  >
                    <Eye v-if="showKey" :size="13" />
                    <EyeOff v-else :size="13" />
                  </button>
                </div>
              </label>
              <label class="text-xs text-app-muted">模型名
                <input
                  v-model="form.model"
                  class="input w-full mt-1 text-xs font-mono"
                  placeholder="deepseek-chat / gpt-4o-mini / glm-4.6 ..."
                  spellcheck="false"
                />
              </label>
              <label class="text-xs text-app-muted">自定义指令（附加到系统提示词，可选）
                <textarea
                  v-model="form.systemPrompt"
                  rows="2"
                  class="input w-full mt-1 text-xs resize-none"
                  placeholder="例如：文档统一用中文，接口示例用 JSON…"
                />
              </label>
              <div class="flex items-center gap-2">
                <button class="btn-ghost btn-sm" :disabled="testing" @click="test">
                  <Loader2 v-if="testing" :size="13" class="animate-spin" />
                  测试连接
                </button>
                <span
                  v-if="testResult"
                  class="text-[11px] truncate"
                  :class="testResult.startsWith('失败') ? 'text-accent-red' : 'text-accent-green'"
                >
                  {{ testResult }}
                </span>
              </div>
              <p class="text-[11px] text-app-muted/80 leading-relaxed">
                兼容任何 OpenAI 格式服务：DeepSeek、GLM、Kimi、OpenAI、本地 Ollama（Base URL 填
                http://localhost:11434/v1）等。密钥仅保存在本地数据库。MCP 服务在顶部 MCP 面板管理。
              </p>
            </div>

            <div class="px-4 py-3 border-t border-app-border flex justify-end gap-2">
              <button class="btn-ghost btn-sm" @click="close">取消</button>
              <button class="btn-primary btn-sm" @click="save">保存</button>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

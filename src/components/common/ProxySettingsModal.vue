<script setup lang="ts">
import { ref, watch } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { Globe, Loader2, ShieldCheck } from "lucide-vue-next";
import { useSettingsStore } from "@/stores/settings";
import { useToast } from "@/composables/useToast";
import type { ProxySettings } from "@/types";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const store = useSettingsStore();
const toast = useToast();

/** 常见代理客户端的本地端口（点击填充，按需改端口） */
const PRESETS = [
  { label: "Clash / Mihomo", url: "http://127.0.0.1:7890" },
  { label: "V2rayN", url: "http://127.0.0.1:10809" },
  { label: "SOCKS5", url: "socks5://127.0.0.1:1080" },
  { label: "公司代理", url: "http://proxy.example.com:8080" },
];

/** 默认测试目标：换一个域名可区分「代理不通」和「目标站不通」 */
const DEFAULT_TEST_URL = "https://www.baidu.com";

const form = ref<ProxySettings>({ enabled: false, url: "", bypass: "" });
const testUrl = ref(DEFAULT_TEST_URL);
const testing = ref(false);
const testResult = ref<{ ok: boolean; text: string } | null>(null);
const saving = ref(false);

watch(
  () => props.show,
  async (show) => {
    if (!show) return;
    if (!store.loaded) await store.load().catch(() => {});
    form.value = { ...store.proxy };
    testUrl.value = DEFAULT_TEST_URL;
    testResult.value = null;
  }
);

function applyPreset(url: string) {
  form.value.url = url;
  form.value.enabled = true;
}

async function test() {
  testing.value = true;
  testResult.value = null;
  try {
    const r = await store.testProxy({ ...form.value }, testUrl.value);
    testResult.value = {
      ok: true,
      text: `连接成功：HTTP ${r.status} · ${r.timeMs}ms（经由 ${form.value.enabled ? "代理" : "直连"}）`,
    };
  } catch (e) {
    testResult.value = { ok: false, text: `失败：${(e as Error).message}` };
  } finally {
    testing.value = false;
  }
}

async function save() {
  saving.value = true;
  try {
    await store.saveProxy({ ...form.value });
    emit("update:show", false);
    toast.push(
      form.value.enabled ? "代理已启用，后续请求立即生效" : "代理已关闭",
      "success"
    );
  } catch (e) {
    // 校验失败（地址非法等）由后端返回，直接展示原因
    toast.push(String(e), "error", 8000);
  } finally {
    saving.value = false;
  }
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
            class="w-[560px] max-w-full bg-app-surface2 border border-app-border rounded-md shadow-2xl"
          >
            <DialogTitle class="px-4 py-3 border-b border-app-border text-sm font-medium flex items-center">
              网络代理
              <button class="ml-auto btn-ghost btn-sm" @click="close">关闭</button>
            </DialogTitle>

            <div class="p-4 flex flex-col gap-3 max-h-[70vh] overflow-auto">
              <div class="flex items-center gap-1.5 text-xs font-semibold text-app-text">
                <Globe :size="13" class="text-accent-blue" /> 出站 HTTP 代理
              </div>

              <!-- 开关 -->
              <label class="flex items-center gap-2 cursor-pointer select-none">
                <input
                  v-model="form.enabled"
                  type="checkbox"
                  class="accent-accent-orange"
                />
                <span class="text-xs text-app-text">启用代理</span>
                <span class="text-[11px] text-app-muted">
                  关闭时按系统环境变量（HTTP_PROXY / HTTPS_PROXY）决定是否走代理
                </span>
              </label>

              <!-- 快捷预设 -->
              <div class="flex items-center gap-1 flex-wrap">
                <span class="text-[11px] text-app-muted">快速填充：</span>
                <button
                  v-for="p in PRESETS"
                  :key="p.label"
                  class="text-[11px] px-2 py-0.5 rounded-md border transition-colors"
                  :class="
                    form.url === p.url
                      ? 'border-accent-blue/60 text-accent-blue bg-accent-blue/10'
                      : 'border-app-border text-app-muted hover:text-app-text hover:border-app-muted'
                  "
                  :title="p.url"
                  @click="applyPreset(p.url)"
                >
                  {{ p.label }}
                </button>
              </div>

              <label class="text-xs text-app-muted">代理地址
                <input
                  v-model="form.url"
                  class="input w-full mt-1 text-xs font-mono"
                  :disabled="!form.enabled"
                  placeholder="http://127.0.0.1:7890 或 socks5://127.0.0.1:1080"
                  spellcheck="false"
                />
              </label>
              <p class="text-[11px] text-app-muted/80 -mt-1.5">
                支持 http / https / socks5，可带账号密码（http://user:pass@host:port）。省略协议时按
                http 处理。
              </p>

              <label class="text-xs text-app-muted">直连名单（不走代理，逗号分隔，可选）
                <input
                  v-model="form.bypass"
                  class="input w-full mt-1 text-xs font-mono"
                  :disabled="!form.enabled"
                  placeholder=".corp.com, 192.168.0.0/16, *.internal"
                  spellcheck="false"
                />
              </label>
              <p class="text-[11px] text-app-muted/80 -mt-1.5 flex items-start gap-1">
                <ShieldCheck :size="12" class="mt-0.5 flex-shrink-0 text-accent-green" />
                <span>
                  本机地址（localhost / 127.0.0.1 / ::1）始终直连，访问本地 Mock 服务和本地
                  Ollama 不受代理影响。
                </span>
              </p>

              <!-- 连通性测试 -->
              <div class="border-t border-app-border pt-3 flex flex-col gap-2">
                <span class="text-[11px] text-app-muted">
                  连通性测试（用上面填写但尚未保存的配置发一次请求）
                </span>
                <div class="flex items-center gap-2">
                  <input
                    v-model="testUrl"
                    class="input flex-1 min-w-0 text-xs font-mono"
                    placeholder="https://www.baidu.com"
                    spellcheck="false"
                  />
                  <button class="btn-ghost btn-sm flex-shrink-0" :disabled="testing" @click="test">
                    <Loader2 v-if="testing" :size="13" class="animate-spin" />
                    测试
                  </button>
                </div>
                <span
                  v-if="testResult"
                  class="text-[11px] leading-relaxed"
                  :class="testResult.ok ? 'text-accent-green' : 'text-accent-red'"
                >
                  {{ testResult.text }}
                </span>
              </div>

              <p class="text-[11px] text-app-muted/80 leading-relaxed">
                代理对所有出站请求生效：发送请求、AI 助手、MCP 工具的联网调用与应用自更新。
                保存后会重建共用连接（已建立的 Cookie 会话与连接池会重置）。
              </p>
            </div>

            <div class="px-4 py-3 border-t border-app-border flex justify-end gap-2">
              <button class="btn-ghost btn-sm" @click="close">取消</button>
              <button class="btn-primary btn-sm" :disabled="saving" @click="save">
                <Loader2 v-if="saving" :size="13" class="animate-spin" />
                保存
              </button>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

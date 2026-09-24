<script setup lang="ts">
import { computed, ref, watch } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import {
  RefreshCw,
  Download,
  CheckCircle2,
  CircleAlert,
  Rocket,
  Settings2,
  Loader2,
} from "lucide-vue-next";
import { useUpdateStore } from "@/stores/update";
import { useToast } from "@/composables/useToast";
import { formatBytes } from "@/utils/format";

/**
 * 应用更新弹窗：检查 / 版本说明 / 下载（进度条）/ 重启安装 / 更新源设置。
 * 打开时自动拉取应用信息并检查一次（手动检查包含被跳过的版本）。
 */
const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const updater = useUpdateStore();
const toast = useToast();

const showSettings = ref(false);
const urlDraft = ref("");

watch(
  () => props.show,
  async (v) => {
    if (!v) return;
    await updater.loadInfo();
    urlDraft.value = updater.info?.manifestUrl ?? "";
    if (updater.status === "idle") {
      await updater.check(true);
    }
  }
);

const percent = computed(() => updater.progress.percent);

async function saveUrl() {
  try {
    await updater.saveManifestUrl(urlDraft.value.trim());
    toast.push("更新源已保存", "success", 2000);
    showSettings.value = false;
    await updater.check(true);
  } catch (e) {
    toast.push(String(e), "error", 5000);
  }
}

const notesLines = computed(() =>
  (updater.manifest?.notes ?? "").split(/\r?\n/).filter((l) => l.trim())
);
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-50" @close="emit('update:show', false)">
      <TransitionChild
        as="template"
        enter="duration-150 ease-out"
        enter-from="opacity-0"
        enter-to="opacity-100"
        leave="duration-100 ease-in"
        leave-from="opacity-100"
        leave-to="opacity-0"
      >
        <div class="fixed inset-0 bg-black/50" />
      </TransitionChild>

      <div class="fixed inset-0 flex items-center justify-center p-4">
        <TransitionChild
          as="template"
          enter="duration-150 ease-out"
          enter-from="opacity-0 scale-95"
          enter-to="opacity-100 scale-100"
          leave="duration-100 ease-in"
          leave-from="opacity-100 scale-100"
          leave-to="opacity-0 scale-95"
        >
          <DialogPanel class="w-[460px] max-w-full pop overflow-hidden">
            <DialogTitle class="px-4 h-10 flex items-center border-b border-app-border text-[13px] font-medium">
              应用更新
              <span v-if="updater.info" class="ml-2 text-[11px] text-app-muted font-mono">
                v{{ updater.info.currentVersion }}
              </span>
              <button
                class="icon-btn ml-auto"
                :class="{ 'text-accent-blue': showSettings }"
                title="更新源设置"
                @click="showSettings = !showSettings"
              >
                <Settings2 :size="14" />
              </button>
              <button class="btn-ghost btn-sm" @click="emit('update:show', false)">关闭</button>
            </DialogTitle>

            <div class="p-4 flex flex-col gap-3 min-h-[180px]">
              <!-- 更新源设置 -->
              <div v-if="showSettings" class="flex flex-col gap-2 border border-app-border rounded-md p-2.5">
                <span class="text-[11px] text-app-muted">
                  更新清单地址（服务器上的 update.json）
                </span>
                <div class="flex gap-2">
                  <input
                    v-model="urlDraft"
                    placeholder="https://your-server/x-apimanage/update.json"
                    spellcheck="false"
                    class="input flex-1 min-w-0 text-xs"
                  />
                  <button class="btn-primary btn-sm" @click="saveUrl">保存</button>
                </div>
                <span v-if="updater.info" class="text-[10px] text-app-muted/70 break-all">
                  安装包下载到：{{ updater.info.dataDir }}\updates
                </span>
              </div>

              <!-- 检查中 -->
              <div v-if="updater.status === 'checking'" class="flex-1 flex flex-col items-center justify-center gap-2 py-6 text-app-muted">
                <Loader2 :size="20" class="animate-spin" />
                <span class="text-xs">正在检查更新…</span>
              </div>

              <!-- 已是最新 -->
              <div v-else-if="updater.status === 'up-to-date'" class="flex-1 flex flex-col items-center justify-center gap-2 py-6">
                <CheckCircle2 :size="22" class="text-accent-green" />
                <span class="text-xs text-app-muted">已是最新版本</span>
                <button class="btn-ghost btn-sm" @click="updater.check(true)">
                  <RefreshCw :size="12" /> 重新检查
                </button>
              </div>

              <!-- 有可用更新 -->
              <template v-else-if="updater.status === 'update-available' && updater.manifest">
                <div class="flex items-center gap-2">
                  <Rocket :size="16" class="text-accent-orange" />
                  <span class="text-sm font-medium">
                    新版本 v{{ updater.manifest.version }}
                  </span>
                  <span class="text-[11px] text-app-muted font-mono">
                    ← v{{ updater.info?.currentVersion }}
                  </span>
                </div>
                <div v-if="notesLines.length" class="max-h-40 overflow-auto bg-app-bg border border-app-border rounded-md p-2.5 text-xs leading-relaxed text-app-text/90 whitespace-pre-wrap">
                  {{ updater.manifest.notes }}
                </div>
                <div class="flex items-center gap-2 justify-end">
                  <button class="btn-ghost btn-sm" @click="updater.skip()">跳过此版本</button>
                  <button class="btn-primary btn-sm" @click="updater.download()">
                    <Download :size="13" /> 下载安装包
                  </button>
                </div>
              </template>

              <!-- 下载中 -->
              <div v-else-if="updater.status === 'downloading'" class="flex-1 flex flex-col items-center justify-center gap-3 py-4">
                <div class="w-full max-w-[320px]">
                  <div class="flex items-center justify-between text-[11px] text-app-muted mb-1.5">
                    <span>正在下载…</span>
                    <span class="font-mono tabular-nums">{{ percent }}%</span>
                  </div>
                  <div class="h-1.5 bg-app-hover rounded-full overflow-hidden">
                    <div
                      class="h-full bg-accent-orange rounded-full transition-[width] duration-200"
                      :style="{ width: percent + '%' }"
                    />
                  </div>
                  <div class="mt-1.5 text-[10px] text-app-muted/70 font-mono text-right">
                    {{ formatBytes(updater.progress.received) }}
                    <template v-if="updater.progress.total"> / {{ formatBytes(updater.progress.total) }}</template>
                  </div>
                </div>
              </div>

              <!-- 已下载，待安装 -->
              <div v-else-if="updater.status === 'downloaded'" class="flex-1 flex flex-col items-center justify-center gap-2.5 py-6">
                <CheckCircle2 :size="22" class="text-accent-green" />
                <span class="text-xs text-app-muted">安装包已就绪，重启应用完成升级</span>
                <button class="btn-primary" @click="updater.install()">
                  <Rocket :size="13" /> 重启并安装
                </button>
              </div>

              <!-- 出错 -->
              <div v-else-if="updater.status === 'error'" class="flex-1 flex flex-col items-center justify-center gap-2.5 py-6">
                <CircleAlert :size="22" class="text-accent-red" />
                <span class="text-xs text-accent-red text-center max-w-[340px] break-all">
                  {{ updater.error }}
                </span>
                <button class="btn-ghost btn-sm" @click="updater.check(true)">
                  <RefreshCw :size="12" /> 重试
                </button>
              </div>

              <!-- 初始 idle -->
              <div v-else class="flex-1 flex flex-col items-center justify-center gap-2 py-6">
                <button class="btn-primary btn-sm" @click="updater.check(true)">
                  <RefreshCw :size="12" /> 检查更新
                </button>
              </div>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

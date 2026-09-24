<script setup lang="ts">
import { computed, ref } from "vue";
import { Terminal, CircleAlert } from "lucide-vue-next";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { useTabStore } from "@/stores/tab";
import { parseCurlCommand, isLikelyCurl } from "@/utils/curlImport";
import type { HttpMethod } from "@/types";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const tabStore = useTabStore();
const text = ref("");

const PLACEHOLDER = [
  "粘贴 cURL 命令，支持 bash / PowerShell / cmd 复制出的格式，例如：",
  `curl -X POST 'https://api.example.com/users' \\`,
  `  -H 'Content-Type: application/json' \\`,
  `  -d '{"name":"test"}'`,
].join("\n");

const parsed = computed(() => {
  const cmd = text.value.trim();
  if (!cmd) return null;
  try {
    return parseCurlCommand(cmd);
  } catch {
    return null;
  }
});

const error = computed(() => {
  const cmd = text.value.trim();
  if (!cmd) return "";
  if (!isLikelyCurl(cmd)) return "未识别到 curl 命令（应以 curl 开头）";
  if (!parsed.value?.url) return "未能解析出 URL";
  return "";
});

function submit() {
  const p = parsed.value;
  if (!p || error.value) return;
  tabStore.openDraft({
    name: p.name,
    method: p.method as HttpMethod,
    url: p.url + (p.params.length ? "?" + p.params.map((k) => `${k.key}=${k.value}`).join("&") : ""),
    params: p.params,
    headers: p.headers,
    body: p.body,
    auth: p.auth,
  });
  close();
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
          <DialogPanel class="w-[560px] max-w-full pop">
            <DialogTitle class="px-4 h-10 flex items-center gap-2 border-b border-app-border text-[13px] font-medium">
              <Terminal :size="14" class="text-accent-orange" />
              导入 cURL 命令
            </DialogTitle>

            <div class="p-4 flex flex-col gap-3">
              <textarea
                v-model="text"
                rows="6"
                spellcheck="false"
                :placeholder="PLACEHOLDER"
                class="w-full input h-auto py-2 font-mono text-xs leading-relaxed resize-none"
                @keydown.enter.ctrl.prevent="submit"
              />

              <!-- 解析预览 -->
              <div v-if="parsed && !error" class="rounded-md bg-app-bg border border-app-border px-3 py-2 text-xs flex flex-col gap-1">
                <div class="flex items-center gap-2">
                  <span class="font-mono font-bold text-[11px] w-12 text-accent-blue">{{ parsed.method }}</span>
                  <span class="font-mono truncate text-app-text/90">{{ parsed.url }}</span>
                </div>
                <div class="flex items-center gap-3 text-[11px] text-app-muted">
                  <span v-if="parsed.params.length">Params {{ parsed.params.length }}</span>
                  <span v-if="parsed.headers.length">Headers {{ parsed.headers.length }}</span>
                  <span v-if="parsed.body">Body（{{ parsed.body.mode }}）</span>
                  <span v-if="parsed.auth">Auth（{{ parsed.auth.type }}）</span>
                </div>
              </div>
              <div v-else-if="error" class="flex items-center gap-1.5 text-xs text-accent-red">
                <CircleAlert :size="13" /> {{ error }}
              </div>

              <div class="flex justify-end gap-2">
                <button class="btn-ghost" @click="close">取消</button>
                <button class="btn-primary" :disabled="!parsed || !!error" @click="submit">
                  导入并打开
                </button>
              </div>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

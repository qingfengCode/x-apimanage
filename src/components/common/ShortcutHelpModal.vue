<script setup lang="ts">
import { computed } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { Keyboard } from "lucide-vue-next";
import { SHORTCUTS } from "@/composables/shortcuts";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const SECTIONS = ["窗口", "请求", "AI"] as const;
const grouped = computed(() =>
  SECTIONS.map((sec) => ({
    section: sec,
    items: SHORTCUTS.filter((s) => s.section === sec),
  }))
);

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
            class="w-[420px] max-w-full bg-app-surface2 border border-app-border rounded-md shadow-2xl"
          >
            <DialogTitle class="px-4 py-3 border-b border-app-border text-sm font-medium flex items-center gap-2">
              <Keyboard :size="14" class="text-accent-purple" /> 快捷键
              <span class="ml-auto text-[11px] font-normal text-app-muted">按 Esc 或 Ctrl+/ 关闭</span>
            </DialogTitle>

            <div class="p-3 flex flex-col gap-3 max-h-[70vh] overflow-auto">
              <section v-for="g in grouped" :key="g.section" v-show="g.items.length">
                <div class="text-[11px] uppercase tracking-wide text-app-muted font-semibold mb-1 px-1">
                  {{ g.section }}
                </div>
                <div class="flex flex-col gap-px">
                  <div
                    v-for="item in g.items"
                    :key="item.label"
                    class="flex items-center justify-between px-2.5 py-1.5 rounded-md text-[13px] hover:bg-app-hover"
                  >
                    <span class="text-app-text">{{ item.label }}</span>
                    <span class="flex items-center gap-1">
                      <kbd
                        v-for="k in item.keys"
                        :key="k"
                        class="px-1.5 py-0.5 rounded bg-app-bg border border-app-border font-mono text-[11px] text-app-text"
                      >
                        {{ k }}
                      </kbd>
                    </span>
                  </div>
                </div>
              </section>
              <p class="text-[11px] text-app-muted px-1">
                macOS 上 Ctrl 对应 Cmd。文本输入框与代码编辑器内部分快捷键不生效（如 Ctrl+/ 是注释）。
              </p>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

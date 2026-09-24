<script setup lang="ts">
import { ref, watch, nextTick } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";

const props = withDefaults(
  defineProps<{
    show: boolean;
    title: string;
    label?: string;
    placeholder?: string;
    initialValue?: string;
    confirmText?: string;
    /** 校验函数：返回错误提示字符串，空串表示通过 */
    validate?: (v: string) => string;
  }>(),
  {
    label: "",
    placeholder: "",
    initialValue: "",
    confirmText: "确定",
    validate: () => "",
  }
);

const emit = defineEmits<{
  (e: "update:show", v: boolean): void;
  (e: "confirm", v: string): void;
  (e: "cancel"): void;
}>();

const value = ref("");
const error = ref("");
const inputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.show,
  async (show) => {
    if (show) {
      value.value = props.initialValue;
      error.value = "";
      await nextTick();
      // 自动聚焦并选中
      inputRef.value?.focus();
      inputRef.value?.select();
    }
  }
);

function submit() {
  const err = props.validate(value.value);
  if (err) {
    error.value = err;
    return;
  }
  error.value = "";
  emit("confirm", value.value.trim());
  emit("update:show", false);
}

function cancel() {
  emit("cancel");
  emit("update:show", false);
}
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-50" @close="cancel">
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
          <DialogPanel
            class="w-[400px] max-w-full pop"
            @keydown.enter="submit"
            @keydown.esc="cancel"
          >
            <DialogTitle class="px-4 h-10 flex items-center border-b border-app-border text-[13px] font-medium">
              {{ title }}
            </DialogTitle>
            <div class="p-4">
              <label v-if="label" class="block text-xs text-app-muted mb-1.5">{{ label }}</label>
              <input
                ref="inputRef"
                v-model="value"
                :placeholder="placeholder"
                spellcheck="false"
                class="w-full input"
                :class="error ? 'border-accent-red' : ''"
              />
              <div v-if="error" class="mt-1.5 text-xs text-accent-red">{{ error }}</div>
              <div class="flex justify-end gap-2 mt-4">
                <button class="btn-ghost btn-sm" @click="cancel">取消</button>
                <button class="btn-primary btn-sm" @click="submit">{{ confirmText }}</button>
              </div>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

<script setup lang="ts">
import { computed } from "vue";
import KeyValueEditor from "@/components/common/KeyValueEditor.vue";
import type { KeyValue } from "@/types";

const props = defineProps<{
  modelValue: KeyValue[];
  url: string;
}>();

const emit = defineEmits<{
  (e: "update:modelValue", v: KeyValue[]): void;
  (e: "update:url", v: string): void;
}>();

// URL 解析：把 query string 同步到 params
const parsedFromUrl = computed(() => {
  try {
    const u = new URL(props.url);
    const out: KeyValue[] = [];
    u.searchParams.forEach((v, k) => out.push({ key: k, value: v, enabled: true }));
    return out;
  } catch {
    return null;
  }
});

function applyFromUrl() {
  const parsed = parsedFromUrl.value;
  if (parsed && parsed.length) {
    emit("update:modelValue", parsed);
  }
}

/** 当 params 变化时同步回 url（重建 query） */
function onParamsChange(items: KeyValue[]) {
  emit("update:modelValue", items);
  // 同步到 URL
  try {
    const u = new URL(props.url);
    u.search = "";
    for (const kv of items) {
      if (kv.enabled && kv.key) u.searchParams.append(kv.key, kv.value);
    }
    emit("update:url", u.toString());
  } catch {
    // URL 无效时不自动重建
  }
}
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="flex items-center justify-between px-3 py-1 text-xs text-app-muted border-b border-app-border/40">
      <span>Query Parameters</span>
      <button
        v-if="parsedFromUrl && parsedFromUrl.length"
        class="text-accent-blue hover:underline"
        @click="applyFromUrl"
      >
        从 URL 同步 ({{ parsedFromUrl.length }})
      </button>
    </div>
    <div class="flex-1 min-h-0">
      <KeyValueEditor
        :model-value="modelValue"
        key-placeholder="参数名"
        value-placeholder="值（可用 {{变量}}）"
        @update:model-value="onParamsChange"
      />
    </div>
  </div>
</template>

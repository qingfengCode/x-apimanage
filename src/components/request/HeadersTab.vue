<script setup lang="ts">
import KeyValueEditor from "@/components/common/KeyValueEditor.vue";
import type { KeyValue } from "@/types";

defineProps<{ modelValue: KeyValue[] }>();
const emit = defineEmits<{ (e: "update:modelValue", v: KeyValue[]): void }>();
</script>

<template>
  <div class="h-full flex flex-col">
    <div class="px-3 py-1 text-xs text-app-muted border-b border-app-border/40">
      HTTP Headers
    </div>
    <div class="flex-1 min-h-0">
      <KeyValueEditor
        :model-value="modelValue"
        key-placeholder="Header 名（如 Content-Type）"
        value-placeholder="值（可用 {{变量}}）"
        @update:model-value="(v) => emit('update:modelValue', v)"
      />
    </div>
  </div>
</template>

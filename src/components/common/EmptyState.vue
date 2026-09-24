<script setup lang="ts">
import type { Component } from "vue";
defineProps<{
  icon?: Component;
  title: string;
  hint?: string;
}>();
</script>

<template>
  <div class="flex flex-col items-center justify-center h-full text-app-muted gap-2.5 px-6 text-center">
    <div
      v-if="icon"
      class="w-12 h-12 rounded-2xl bg-app-surface2 border border-app-border/60 flex items-center justify-center"
    >
      <component :is="icon" :size="22" :stroke-width="1.3" class="text-app-muted/80" />
    </div>
    <div class="text-[13px] text-app-text/80">{{ title }}</div>
    <div v-if="hint" class="text-xs text-app-muted/70 max-w-xs leading-relaxed">{{ hint }}</div>
    <!-- 可选操作区（如「导入 cURL」） -->
    <slot name="actions" />
  </div>
</template>

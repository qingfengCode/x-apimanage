<script setup lang="ts">
import { CheckCircle2, Info, AlertCircle, X } from "lucide-vue-next";
import { useToast } from "@/composables/useToast";

const { toasts, dismiss } = useToast();

const typeStyles: Record<string, string> = {
  success: "border-accent-green/50 text-accent-green",
  error: "border-accent-red/50 text-accent-red",
  info: "border-accent-blue/50 text-accent-blue",
};
</script>

<template>
  <div class="fixed bottom-4 right-4 z-[100] flex flex-col gap-2 w-80 max-w-[calc(100vw-2rem)] pointer-events-none">
    <TransitionGroup
      enter-active-class="transition duration-200 ease-out"
      enter-from-class="opacity-0 translate-y-2"
      enter-to-class="opacity-100 translate-y-0"
      leave-active-class="transition duration-150 ease-in"
      leave-from-class="opacity-100"
      leave-to-class="opacity-0 translate-y-1"
    >
      <div
        v-for="t in toasts"
        :key="t.id"
        class="pointer-events-auto flex items-start gap-2.5 px-3 py-2 bg-app-surface2 border rounded-lg text-xs text-app-text"
        :class="typeStyles[t.type]"
        style="box-shadow: var(--shadow-pop)"
        role="status"
      >
        <CheckCircle2 v-if="t.type === 'success'" :size="14" class="mt-0.5 flex-shrink-0" />
        <AlertCircle v-else-if="t.type === 'error'" :size="14" class="mt-0.5 flex-shrink-0" />
        <Info v-else :size="14" class="mt-0.5 flex-shrink-0" />
        <span class="flex-1 min-w-0 break-words py-0.5">{{ t.message }}</span>
        <button
          v-if="t.action"
          class="btn-ghost btn-sm flex-shrink-0"
          @click="t.action.onClick(); dismiss(t.id)"
        >
          {{ t.action.label }}
        </button>
        <button class="flex-shrink-0 opacity-50 hover:opacity-100 mt-0.5" @click="dismiss(t.id)">
          <X :size="13" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

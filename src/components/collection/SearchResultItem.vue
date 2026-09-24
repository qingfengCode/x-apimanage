<script setup lang="ts">
import { Send, Folder } from "lucide-vue-next";
import type { Collection, RequestItem } from "@/types";
import { useTabStore } from "@/stores/tab";
import { methodColor } from "@/utils/format";

const props = defineProps<{ collection: Collection; request: RequestItem }>();
const tabStore = useTabStore();

function open() {
  tabStore.openRequest(props.request);
}
</script>

<template>
  <div
    class="flex items-center gap-1.5 px-3 py-1 cursor-pointer rounded text-sm hover:bg-app-hover"
    @click="open"
  >
    <Send :size="12" class="flex-shrink-0" :class="methodColor(request.method)" />
    <span
      class="font-mono text-[11px] font-bold w-[42px] flex-shrink-0"
      :class="methodColor(request.method)"
    >
      {{ (request.method || "GET").slice(0, 4) }}
    </span>
    <span class="truncate text-app-text/90">{{ request.name }}</span>
    <span class="ml-auto flex items-center gap-1 text-[10px] text-app-muted flex-shrink-0">
      <Folder :size="10" />
      <span class="max-w-[80px] truncate">{{ collection.name }}</span>
    </span>
  </div>
</template>

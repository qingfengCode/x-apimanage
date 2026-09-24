<script setup lang="ts">
import { ref, watch } from "vue";
import { CheckCircle2, XCircle, Terminal, AlertTriangle, ChevronDown } from "lucide-vue-next";
import type { ScriptRunResult } from "@/types";

const props = defineProps<{ result: ScriptRunResult | null }>();

const collapsed = ref(true);

// 新结果到达：有测试/错误时自动展开，否则收起
watch(
  () => props.result,
  (r) => {
    if (!r) return;
    collapsed.value = !(r.tests.length || r.preError || r.testError);
  }
);
</script>

<template>
  <div v-if="result" class="border-t border-app-border bg-app-surface max-h-[40%] flex flex-col">
    <!-- 汇总条（点击折叠/展开） -->
    <button
      class="flex items-center gap-3 px-3 py-1.5 border-b border-app-border text-xs text-left hover:bg-app-hover/60 transition-colors"
      title="点击折叠/展开"
      @click="collapsed = !collapsed"
    >
      <ChevronDown
        :size="13"
        class="text-app-muted flex-shrink-0 transition-transform"
        :class="collapsed ? '-rotate-90' : ''"
      />
      <span class="text-app-muted">Tests:</span>
      <span class="text-accent-green flex items-center gap-1">
        <CheckCircle2 :size="13" />
        {{ result.tests.filter((t) => t.passed).length }} 通过
      </span>
      <span class="text-accent-red flex items-center gap-1" v-if="result.tests.some((t) => !t.passed)">
        <XCircle :size="13" />
        {{ result.tests.filter((t) => !t.passed).length }} 失败
      </span>
      <span v-if="result.preError || result.testError" class="text-accent-yellow flex items-center gap-1 ml-auto">
        <AlertTriangle :size="13" /> 脚本异常
      </span>
    </button>

    <div v-show="!collapsed" class="overflow-auto">
      <!-- 测试结果列表 -->
      <div
        v-for="(t, i) in result.tests"
        :key="i"
        class="flex items-start gap-2 px-3 py-1 border-b border-app-border/30 text-xs"
      >
        <component
          :is="t.passed ? CheckCircle2 : XCircle"
          :size="13"
          class="mt-0.5 flex-shrink-0"
          :class="t.passed ? 'text-accent-green' : 'text-accent-red'"
        />
        <span class="text-app-text">{{ t.name }}</span>
        <span v-if="t.error" class="text-app-muted ml-1 font-mono text-[11px]">— {{ t.error }}</span>
      </div>

      <!-- 脚本错误 -->
      <div v-if="result.preError" class="px-3 py-1.5 text-xs text-accent-yellow border-b border-app-border/30">
        Pre-Script 错误：{{ result.preError }}
      </div>
      <div v-if="result.testError" class="px-3 py-1.5 text-xs text-accent-yellow border-b border-app-border/30">
        Test-Script 错误：{{ result.testError }}
      </div>

      <!-- console 日志 -->
      <div v-if="result.logs.length" class="px-3 py-1.5">
        <div class="text-[11px] text-app-muted flex items-center gap-1 mb-1">
          <Terminal :size="11" /> Console
        </div>
        <pre class="text-[11px] font-mono text-app-text/70 whitespace-pre-wrap">{{ result.logs.join("\n") }}</pre>
      </div>
    </div>
  </div>
</template>

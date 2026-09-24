<script setup lang="ts">
import { ref } from "vue";
import MonacoEditor from "@/components/editor/MonacoEditor.vue";
import { Info } from "lucide-vue-next";

const props = defineProps<{
  preScript: string;
  testScript: string;
}>();
const emit = defineEmits<{
  (e: "update:preScript", v: string): void;
  (e: "update:testScript", v: string): void;
}>();

const active = ref<"pre" | "test">("pre");

const PRE_SNIPPETS = [
  "// Pre-Request Script (在发送前执行)",
  "// 可用：pm.environment.get/set(key,value)、pm.variables.get/set",
  "// 例：根据环境动态生成 token",
  "const user = pm.environment.get('user');",
  "pm.environment.set('token', 'generated-for-' + user);",
].join("\n");

const TEST_SNIPPETS = [
  "// Tests (在收到响应后执行)",
  "// 可用：pm.response.json()/text()/code、pm.test、pm.expect",
  "pm.test('状态码为 200', () => {",
  "  pm.expect(pm.response.code).to.equal(200);",
  "});",
  "pm.test('响应包含预期字段', () => {",
  "  const body = pm.response.json();",
  "  pm.expect(body).to.have.property('id');",
  "});",
].join("\n");
</script>

<template>
  <div class="h-full flex flex-col">
    <!-- 切换 -->
    <div class="flex items-center gap-1 px-3 py-1.5 border-b border-app-border/40 text-xs">
      <button
        class="seg"
        :class="active === 'pre' ? 'seg-active' : ''"
        @click="active = 'pre'"
      >
        Pre-Request
      </button>
      <button
        class="seg"
        :class="active === 'test' ? 'seg-active' : ''"
        @click="active = 'test'"
      >
        Post-Response (Tests)
      </button>
      <span class="ml-auto text-[11px] text-app-muted flex items-center gap-1">
        <Info :size="11" /> 基于 QuickJS 沙箱执行
      </span>
    </div>

    <div class="flex-1 min-h-0">
      <div v-show="active === 'pre'" class="h-full flex flex-col">
        <div class="flex-1 min-h-0">
          <MonacoEditor
            :model-value="props.preScript || ''"
            language="javascript"
            @update:model-value="(v: string) => emit('update:preScript', v)"
          />
        </div>
        <button
          v-if="!props.preScript"
          class="text-[11px] text-accent-blue hover:underline px-3 py-1 text-left"
          @click="emit('update:preScript', PRE_SNIPPETS)"
        >
          插入示例代码
        </button>
      </div>

      <div v-show="active === 'test'" class="h-full flex flex-col">
        <div class="flex-1 min-h-0">
          <MonacoEditor
            :model-value="props.testScript || ''"
            language="javascript"
            @update:model-value="(v: string) => emit('update:testScript', v)"
          />
        </div>
        <button
          v-if="!props.testScript"
          class="text-[11px] text-accent-blue hover:underline px-3 py-1 text-left"
          @click="emit('update:testScript', TEST_SNIPPETS)"
        >
          插入示例代码
        </button>
      </div>
    </div>
  </div>
</template>

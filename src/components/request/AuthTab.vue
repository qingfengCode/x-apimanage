<script setup lang="ts">
import { computed } from "vue";
import { ShieldCheck, Info } from "lucide-vue-next";
import type { AuthConfig } from "@/types";

const props = defineProps<{ modelValue: AuthConfig | null }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: AuthConfig | null): void;
}>();

type AuthType = "none" | "bearer" | "basic" | "apikey";

const TYPES: { key: AuthType; label: string }[] = [
  { key: "none", label: "无" },
  { key: "bearer", label: "Bearer Token" },
  { key: "basic", label: "Basic Auth" },
  { key: "apikey", label: "API Key" },
];

const currentType = computed<AuthType>(() => props.modelValue?.type ?? "none");

/** 提示文案里的变量占位符字面量（模板内嵌 {{}} 会被解析器误判，放脚本里） */
const VAR_HINT = "{{变量}}";

function switchType(t: AuthType) {
  switch (t) {
    case "none":
      emit("update:modelValue", null);
      break;
    case "bearer":
      // 保留同类型已填字段，切换其它类型时从零开始
      emit("update:modelValue", {
        type: "bearer",
        token: props.modelValue?.type === "bearer" ? props.modelValue.token : "",
        prefix: props.modelValue?.type === "bearer" ? props.modelValue.prefix : "Bearer",
      });
      break;
    case "basic":
      emit("update:modelValue", {
        type: "basic",
        username: props.modelValue?.type === "basic" ? props.modelValue.username : "",
        password: props.modelValue?.type === "basic" ? props.modelValue.password : "",
      });
      break;
    case "apikey":
      emit("update:modelValue", {
        type: "apikey",
        addTo: "header",
        key: props.modelValue?.type === "apikey" ? props.modelValue.key : "X-API-Key",
        value: props.modelValue?.type === "apikey" ? props.modelValue.value : "",
      });
      break;
  }
}

function patch(p: Record<string, string>) {
  const cur = props.modelValue;
  if (!cur) return;
  emit("update:modelValue", { ...cur, ...p } as AuthConfig);
}

/** 发送时将注入的内容预览（不解析变量，仅示意） */
const injectPreview = computed<string>(() => {
  const a = props.modelValue;
  if (!a) return "";
  if (a.type === "bearer") {
    const scheme = a.prefix.trim() || "Bearer";
    return `${scheme} ${a.token || "<token>"}`;
  }
  if (a.type === "basic") {
    return `Authorization: Basic base64(${a.username || "<user>"}:${a.password || "<pass>"})`;
  }
  return a.addTo === "header"
    ? `${a.key}: ${a.value || "<value>"}`
    : `?${a.key}=${a.value || "<value>"}`;
});
</script>

<template>
  <div class="h-full flex flex-col">
    <!-- 类型切换 -->
    <div class="flex items-center gap-1 px-2.5 h-8 border-b border-app-border/60 text-xs">
      <button
        v-for="t in TYPES"
        :key="t.key"
        class="seg"
        :class="currentType === t.key ? 'seg-active' : ''"
        @click="switchType(t.key)"
      >
        {{ t.label }}
      </button>
      <span class="ml-auto text-[10px] text-app-muted/70 hidden md:inline">字段支持 {{ VAR_HINT }}</span>
    </div>

    <!-- 表单 -->
    <div class="flex-1 overflow-auto p-3">
      <div v-if="!modelValue" class="h-full flex flex-col items-center justify-center text-app-muted gap-2">
        <ShieldCheck :size="22" :stroke-width="1.3" class="opacity-50" />
        <span class="text-xs">此请求未使用认证，凭据将通过 Headers 手动携带</span>
      </div>

      <!-- Bearer -->
      <div v-else-if="modelValue.type === 'bearer'" class="flex flex-col gap-2.5 max-w-md">
        <label class="flex flex-col gap-1">
          <span class="text-[11px] text-app-muted">Token（发送时自动拼接为「{{ modelValue.prefix || "Bearer" }} &lt;token&gt;」请求头）</span>
          <input
            :value="modelValue.token"
            placeholder="eyJhbGciOi... 或 {{accessToken}}"
            spellcheck="false"
            class="input font-mono"
            @input="patch({ token: ($event.target as HTMLInputElement).value })"
          />
        </label>
        <label class="flex flex-col gap-1 w-40">
          <span class="text-[11px] text-app-muted">前缀（Scheme）</span>
          <input
            :value="modelValue.prefix"
            placeholder="Bearer"
            spellcheck="false"
            class="input"
            @input="patch({ prefix: ($event.target as HTMLInputElement).value })"
          />
        </label>
      </div>

      <!-- Basic -->
      <div v-else-if="modelValue.type === 'basic'" class="flex flex-col gap-2.5 max-w-md">
        <label class="flex flex-col gap-1">
          <span class="text-[11px] text-app-muted">用户名</span>
          <input
            :value="modelValue.username"
            placeholder="user 或 {{username}}"
            spellcheck="false"
            class="input"
            @input="patch({ username: ($event.target as HTMLInputElement).value })"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-[11px] text-app-muted">密码</span>
          <input
            type="password"
            :value="modelValue.password"
            placeholder="•••••• 或 {{password}}"
            spellcheck="false"
            class="input"
            @input="patch({ password: ($event.target as HTMLInputElement).value })"
          />
        </label>
      </div>

      <!-- API Key -->
      <div v-else-if="modelValue.type === 'apikey'" class="flex flex-col gap-2.5 max-w-md">
        <div class="flex gap-2">
          <label class="flex-1 flex flex-col gap-1">
            <span class="text-[11px] text-app-muted">Key</span>
            <input
              :value="modelValue.key"
              placeholder="X-API-Key"
              spellcheck="false"
              class="input"
              @input="patch({ key: ($event.target as HTMLInputElement).value })"
            />
          </label>
          <label class="flex-1 flex flex-col gap-1">
            <span class="text-[11px] text-app-muted">Value</span>
            <input
              :value="modelValue.value"
              placeholder="abc123 或 {{apiKey}}"
              spellcheck="false"
              class="input"
              @input="patch({ value: ($event.target as HTMLInputElement).value })"
            />
          </label>
        </div>
        <div class="flex flex-col gap-1">
          <span class="text-[11px] text-app-muted">添加到</span>
          <div class="flex gap-1">
            <button class="seg" :class="modelValue.addTo === 'header' ? 'seg-active' : ''" @click="patch({ addTo: 'header' })">
              请求头 Header
            </button>
            <button class="seg" :class="modelValue.addTo === 'query' ? 'seg-active' : ''" @click="patch({ addTo: 'query' })">
              查询参数 Query
            </button>
          </div>
        </div>
      </div>

      <!-- 注入预览 -->
      <div v-if="modelValue" class="mt-4 flex items-center gap-1.5 text-[11px] text-app-muted">
        <Info :size="12" class="flex-shrink-0" />
        <span>发送时注入：</span>
        <code class="font-mono text-accent-green/90 bg-app-surface2 border border-app-border rounded px-1.5 py-0.5 truncate max-w-[320px]">
          {{ injectPreview }}
        </code>
      </div>
    </div>
  </div>
</template>

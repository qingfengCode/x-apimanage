<script setup lang="ts">
import { computed, ref } from "vue";
import KeyValueEditor from "@/components/common/KeyValueEditor.vue";
import MonacoEditor from "@/components/editor/MonacoEditor.vue";
import type { RequestBody, KeyValue } from "@/types";

const props = defineProps<{ modelValue: RequestBody | null }>();
const emit = defineEmits<{
  (e: "update:modelValue", v: RequestBody | null): void;
}>();

type BodyType = "none" | "raw" | "form" | "multipart";

const type = computed<BodyType>(() => {
  if (!props.modelValue) return "none";
  return props.modelValue.mode;
});

const rawLang = computed(() => {
  if (props.modelValue?.mode !== "raw") return "text";
  const mt = props.modelValue.mimeType.toLowerCase();
  if (mt.includes("json")) return "json";
  if (mt.includes("html")) return "html";
  if (mt.includes("xml")) return "xml";
  if (mt.includes("javascript")) return "javascript";
  return "text";
});

const RAW_LANGUAGES = [
  { label: "JSON", mime: "application/json" },
  { label: "Text", mime: "text/plain" },
  { label: "JavaScript", mime: "application/javascript" },
  { label: "HTML", mime: "text/html" },
  { label: "XML", mime: "application/xml" },
];

function setType(t: BodyType) {
  const prev = props.modelValue;
  if (t === "none") {
    emit("update:modelValue", null);
  } else if (t === "raw") {
    emit("update:modelValue", {
      mode: "raw",
      raw: prev?.mode === "raw" ? prev.raw : "",
      mimeType: prev?.mode === "raw" ? prev.mimeType : "application/json",
    });
  } else if (t === "form" || t === "multipart") {
    // form 与 multipart 结构完全相同（都是 items），互转应无损保留已有字段
    const items =
      prev?.mode === "form" || prev?.mode === "multipart" ? prev.items : [];
    emit("update:modelValue", { mode: t, items });
  }
}

function setRawLang(mime: string) {
  if (props.modelValue?.mode !== "raw") return;
  emit("update:modelValue", { ...props.modelValue, mimeType: mime });
}

function onLangChange(e: Event) {
  setRawLang((e.target as HTMLSelectElement).value);
}

function setRawText(raw: string) {
  if (props.modelValue?.mode !== "raw") return;
  emit("update:modelValue", { ...props.modelValue, raw });
}

function setFormItems(items: KeyValue[]) {
  if (props.modelValue?.mode === "form") {
    emit("update:modelValue", { ...props.modelValue, items });
  } else if (props.modelValue?.mode === "multipart") {
    emit("update:modelValue", { ...props.modelValue, items });
  }
}

function formatJson() {
  if (props.modelValue?.mode !== "raw") return;
  try {
    const obj = JSON.parse(props.modelValue.raw);
    setRawText(JSON.stringify(obj, null, 2));
  } catch {
    /* ignore */
  }
}

const editorRef = ref();

/** 当前表单/字段项（仅 form/multipart 模式） */
const kvItems = computed<KeyValue[]>(() => {
  if (props.modelValue && (props.modelValue.mode === "form" || props.modelValue.mode === "multipart")) {
    return props.modelValue.items;
  }
  return [];
});
</script>

<template>
  <div class="h-full flex flex-col">
    <!-- 类型切换 -->
    <div class="flex items-center gap-1 px-3 py-1.5 border-b border-app-border/40 text-xs">
      <button
        v-for="t in (['none', 'raw', 'form', 'multipart'] as const)"
        :key="t"
        class="seg"
        :class="type === t ? 'seg-active' : ''"
        @click="setType(t)"
      >
        {{ t === "form" ? "x-www-form-urlencoded" : t === "none" ? "none" : t }}
      </button>

      <!-- raw 语言选择 -->
      <template v-if="type === 'raw'">
        <span class="text-app-muted ml-2">|</span>
        <select
          class="bg-transparent text-xs outline-none text-app-muted"
          :value="modelValue?.mode === 'raw' ? modelValue.mimeType : ''"
          @change="onLangChange"
        >
          <option v-for="l in RAW_LANGUAGES" :key="l.mime" :value="l.mime">{{ l.label }}</option>
        </select>
        <button
          v-if="rawLang === 'json'"
          class="ml-auto text-accent-blue hover:underline"
          @click="formatJson"
        >
          美化 JSON
        </button>
      </template>
    </div>

    <!-- 内容 -->
    <div class="flex-1 min-h-0">
      <div v-if="type === 'none'" class="h-full flex items-center justify-center text-app-muted text-sm">
        该请求没有 body
      </div>

      <div v-else-if="type === 'raw'" class="h-full">
        <MonacoEditor
          ref="editorRef"
          :model-value="modelValue?.mode === 'raw' ? modelValue.raw : ''"
          :language="rawLang"
          placeholder="在此输入请求体..."
          @update:model-value="setRawText"
        />
      </div>

      <div v-else class="h-full">
        <KeyValueEditor
          :model-value="kvItems"
          key-placeholder="字段名"
          value-placeholder="值（可用 {{变量}}）"
          @update:model-value="setFormItems"
        />
      </div>
    </div>
  </div>
</template>

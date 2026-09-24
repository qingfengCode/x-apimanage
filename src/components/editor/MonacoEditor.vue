<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api";
// 注册常用语言的语法高亮（basic-languages）
// 注意：json / typescript / css / html 由 language service 提供完整支持（含 worker）
import "monaco-editor/esm/vs/basic-languages/xml/xml.contribution";
import "monaco-editor/esm/vs/basic-languages/markdown/markdown.contribution";
import "monaco-editor/esm/vs/basic-languages/yaml/yaml.contribution";
// 启用 json / html / css / typescript 的 language service（补全、校验、格式化）
import "monaco-editor/esm/vs/language/json/monaco.contribution";
import "monaco-editor/esm/vs/language/html/monaco.contribution";
import "monaco-editor/esm/vs/language/css/monaco.contribution";
import "monaco-editor/esm/vs/language/typescript/monaco.contribution";

// 配置 web worker（Vite 原生 ?worker 支持）
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

// @ts-ignore - MonacoEnvironment 是 monaco 的全局约定
self.MonacoEnvironment = {
  getWorker(_workerId: string, label: string) {
    switch (label) {
      case "json":
        return new jsonWorker();
      case "css":
      case "scss":
      case "less":
        return new cssWorker();
      case "html":
      case "handlebars":
      case "razor":
        return new htmlWorker();
      case "typescript":
      case "javascript":
        return new tsWorker();
      default:
        return new editorWorker();
    }
  },
};

const props = withDefaults(
  defineProps<{
    modelValue?: string;
    language?: string;
    readOnly?: boolean;
    placeholder?: string;
    minimap?: boolean;
    lineNumbers?: boolean;
    wordWrap?: boolean;
  }>(),
  {
    modelValue: "",
    language: "json",
    readOnly: false,
    minimap: false,
    lineNumbers: true,
    wordWrap: true,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", v: string): void;
  (e: "change"): void;
}>();

const containerRef = ref<HTMLDivElement | null>(null);
const editor = shallowRef<monaco.editor.IStandaloneCodeEditor | null>(null);

onMounted(() => {
  if (!containerRef.value) return;
  // 深色主题（背景透明，跟随容器配色）
  monaco.editor.defineTheme("x-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [],
    colors: {
      "editor.background": "#00000000",
      "editorGutter.background": "#00000000",
    },
  });

  editor.value = monaco.editor.create(containerRef.value, {
    value: props.modelValue,
    language: props.language,
    theme: "x-dark",
    readOnly: props.readOnly,
    minimap: { enabled: props.minimap },
    lineNumbers: props.lineNumbers ? "on" : "off",
    wordWrap: props.wordWrap ? "on" : "off",
    automaticLayout: true,
    fontSize: 12.5,
    fontFamily: "'JetBrains Mono', Menlo, Consolas, monospace",
    scrollBeyondLastLine: false,
    renderWhitespace: "none",
    tabSize: 2,
    padding: { top: 6, bottom: 6 },
    scrollbar: { verticalScrollbarSize: 8, horizontalScrollbarSize: 8 },
  });

  editor.value.onDidChangeModelContent(() => {
    const v = editor.value?.getValue() ?? "";
    emit("update:modelValue", v);
    emit("change");
  });
});

// 外部 modelValue 变化时同步（避免光标跳到末尾：仅当不同时设置）
watch(
  () => props.modelValue,
  (val) => {
    if (editor.value && editor.value.getValue() !== val) {
      editor.value.setValue(val ?? "");
    }
  }
);

watch(
  () => props.language,
  (lang) => {
    if (editor.value) {
      const model = editor.value.getModel();
      if (model) monaco.editor.setModelLanguage(model, lang);
    }
  }
);

watch(
  () => props.readOnly,
  (ro) => editor.value?.updateOptions({ readOnly: ro })
);

onBeforeUnmount(() => {
  editor.value?.dispose();
  editor.value = null;
});

defineExpose({
  format: () => {
    if (!editor.value) return;
    editor.value.getAction("editor.action.formatDocument")?.run();
  },
  getEditor: () => editor.value,
});
</script>

<template>
  <div ref="containerRef" class="w-full h-full" />
</template>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { Plus, X, GripVertical } from "lucide-vue-next";
import type { KeyValue } from "@/types";
import { uid } from "@/utils/id";

const props = withDefaults(
  defineProps<{
    modelValue: KeyValue[];
    keyPlaceholder?: string;
    valuePlaceholder?: string;
    showToggle?: boolean;
    allowBulk?: boolean;
  }>(),
  {
    keyPlaceholder: "key",
    valuePlaceholder: "value",
    showToggle: true,
    allowBulk: false,
  }
);

const emit = defineEmits<{
  (e: "update:modelValue", v: KeyValue[]): void;
  (e: "change"): void;
}>();

const rows = computed(() => props.modelValue);

// 稳定的行 key：与 modelValue 等长，增删时维护，避免用 index 导致 input 状态错乱
const rowKeys = ref<string[]>([]);
function syncKeys() {
  const n = rows.value.length;
  if (rowKeys.value.length < n) {
    while (rowKeys.value.length < n) rowKeys.value.push(uid("kv"));
  } else if (rowKeys.value.length > n) {
    rowKeys.value.length = n;
  }
}
syncKeys();
watch(rows, syncKeys, { deep: false });

function update(idx: number, patch: Partial<KeyValue>) {
  const next = rows.value.map((r, i) => (i === idx ? { ...r, ...patch } : r));
  emit("update:modelValue", next);
  emit("change");
}

function addRow() {
  // 先 push key，再 emit（emit 会触发 sync 补齐 key）
  rowKeys.value.push(uid("kv"));
  const next = [...rows.value, { key: "", value: "", enabled: true }];
  emit("update:modelValue", next);
}

function removeRow(idx: number) {
  rowKeys.value.splice(idx, 1);
  const next = rows.value.filter((_, i) => i !== idx);
  emit("update:modelValue", next);
  emit("change");
}

/** 确保始终有一个空行便于输入 */
function ensureTrailing() {
  const last = rows.value[rows.value.length - 1];
  if (!last || last.key || last.value) addRow();
}
ensureTrailing();

// ---- 批量粘贴：支持 Header 式 `key: value`、URL 参数式 `key=value`、tab 分隔列 ----
function parseKvLine(line: string): KeyValue {
  const s = line.trim();
  const tabIdx = s.indexOf("\t");
  if (tabIdx > 0) {
    return { key: s.slice(0, tabIdx).trim(), value: s.slice(tabIdx + 1).trim(), enabled: true };
  }
  const colonIdx = s.indexOf(":");
  if (colonIdx > 0) {
    return { key: s.slice(0, colonIdx).trim(), value: s.slice(colonIdx + 1).trim(), enabled: true };
  }
  const eqIdx = s.indexOf("=");
  if (eqIdx > 0) {
    return { key: s.slice(0, eqIdx).trim(), value: s.slice(eqIdx + 1).trim(), enabled: true };
  }
  return { key: s, value: "", enabled: true };
}

function onTablePaste(e: ClipboardEvent) {
  const text = e.clipboardData?.getData("text/plain") ?? "";
  if (!text.includes("\n") && !text.includes("\t") && !/[:=]/.test(text)) {
    return; // 单行普通内容：交给输入框默认行为
  }
  // 多行 / 明显带分隔符：按行解析追加，替换尾部空行
  e.preventDefault();
  const parsed = text
    .split(/\r?\n/)
    .filter((l) => l.trim() !== "")
    .map(parseKvLine);
  if (!parsed.length) return;
  const kept = rows.value.filter((r) => r.key || r.value);
  emit("update:modelValue", [...kept, ...parsed]);
  emit("change");
}

// ---- 行拖拽排序：按住手柄的行才可拖动（避免与输入框文本选择冲突） ----
const dragIdx = ref<number | null>(null);
const overIdx = ref<number | null>(null);

function onRowDragStart(idx: number, e: DragEvent) {
  if (e.dataTransfer) {
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", String(idx));
  }
}

function onRowDragOver(idx: number, e: DragEvent) {
  if (dragIdx.value === null) return;
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  overIdx.value = idx;
}

function onRowDrop(idx: number) {
  const from = dragIdx.value;
  if (from === null || from === idx) {
    resetRowDrag();
    return;
  }
  // rows 与 rowKeys 索引对齐，必须同步移动，否则行 key 与数据错位导致输入状态混乱
  const next = rows.value.map((r) => ({ ...r }));
  const nextKeys = rowKeys.value.slice();
  const [moved] = next.splice(from, 1);
  const [movedKey] = nextKeys.splice(from, 1);
  next.splice(idx, 0, moved);
  nextKeys.splice(idx, 0, movedKey);
  rowKeys.value = nextKeys;
  emit("update:modelValue", next);
  emit("change");
  resetRowDrag();
}

function resetRowDrag() {
  dragIdx.value = null;
  overIdx.value = null;
}
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="flex-1 overflow-auto" @paste="onTablePaste">
      <table class="w-full">
        <tbody>
          <tr
            v-for="(row, idx) in rows"
            :key="rowKeys[idx]"
            :draggable="dragIdx === idx"
            class="group border-b border-app-border/40 hover:bg-app-hover/40 focus-within:bg-app-hover/40 transition-colors"
            :class="{
              'opacity-50': (showToggle && !row.enabled) || dragIdx === idx,
              'drag-over-top': overIdx === idx && dragIdx !== null && dragIdx !== idx,
            }"
            @dragstart="(e) => onRowDragStart(idx, e)"
            @dragover="(e) => onRowDragOver(idx, e)"
            @drop.prevent="onRowDrop(idx)"
            @dragend="resetRowDrag"
            @mouseup="resetRowDrag"
          >
            <!-- 拖拽手柄：按住时才激活整行拖动 -->
            <td class="w-5 pl-1.5 select-none">
              <span
                class="inline-flex items-center justify-center w-4 h-6 text-app-muted/0 group-hover:text-app-muted/70 cursor-grab active:cursor-grabbing"
                title="拖动排序"
                @mousedown="dragIdx = idx"
              >
                <GripVertical :size="12" />
              </span>
            </td>
            <!-- 启用勾选 -->
            <td class="w-8 px-2 text-center" v-if="showToggle">
              <input
                type="checkbox"
                :checked="row.enabled"
                @change="update(idx, { enabled: !row.enabled })"
                class="accent-accent-orange cursor-pointer"
              />
            </td>
            <!-- key -->
            <td class="px-1">
              <input
                :value="row.key"
                :placeholder="keyPlaceholder"
                spellcheck="false"
                @input="update(idx, { key: ($event.target as HTMLInputElement).value })"
                class="w-full bg-transparent px-1.5 h-6 outline-none focus:bg-app-surface2 focus:rounded text-app-text font-mono text-xs transition-colors"
              />
            </td>
            <td class="w-px bg-app-border/40"></td>
            <!-- value -->
            <td class="px-1">
              <input
                :value="row.value"
                :placeholder="valuePlaceholder"
                spellcheck="false"
                @input="update(idx, { value: ($event.target as HTMLInputElement).value })"
                class="w-full bg-transparent px-1.5 h-6 outline-none focus:bg-app-surface2 focus:rounded text-app-text font-mono text-xs transition-colors"
              />
            </td>
            <!-- 删除 -->
            <td class="w-7 px-1 text-center">
              <button
                v-if="row.key || row.value"
                class="w-5 h-5 inline-flex items-center justify-center rounded text-app-muted hover:text-accent-red hover:bg-accent-red/10 opacity-0 group-hover:opacity-100 transition-all"
                @click="removeRow(idx)"
                title="删除"
              >
                <X :size="12" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="border-t border-app-border px-2 h-7 flex items-center gap-2 flex-shrink-0">
      <button class="btn-ghost btn-sm" @click="addRow">
        <Plus :size="13" /> 添加
      </button>
      <span class="text-[10px] text-app-muted/70 hidden lg:inline">支持直接粘贴多行：key: value / key=value</span>
      <span class="text-[11px] text-app-muted ml-auto">
        {{ rows.filter((r) => r.enabled && r.key).length }} 项
      </span>
    </div>
  </div>
</template>

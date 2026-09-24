<script setup lang="ts">
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { Download, Upload, Check, FileJson } from "lucide-vue-next";
import { useCollectionStore } from "@/stores/collection";
import {
  parsePostmanCollection,
  requestItemToPmItem,
  buildPostmanCollectionTree,
} from "@/utils/postman";
import { parseOpenApiText } from "@/utils/openapiImport";
import type { RequestItem, Collection } from "@/types";
import type { PmItem } from "@/types/postman";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const collectionStore = useCollectionStore();

const tab = ref<"postman" | "openapi">("postman");

const importing = ref(false);
const importMsg = ref<{ ok: boolean; text: string } | null>(null);
const exporting = ref(false);
const exportMsg = ref<{ ok: boolean; text: string } | null>(null);

const oaImporting = ref(false);
const oaMsg = ref<{ ok: boolean; text: string } | null>(null);

// 导出：选择集合
const exportColId = ref<string>("");
const collections = computed(() =>
  collectionStore.collections.filter((c) => c.kind === "collection")
);

/** 统计 item 树中的请求数量（文件夹本身不计） */
function countRequests(items: PmItem[]): number {
  return items.reduce(
    (n, it) => n + (it.request ? 1 : 0) + (it.item ? countRequests(it.item) : 0),
    0
  );
}

/** 按树递归组织某节点下的导出内容：先子文件夹（递归），后本层请求 */
function exportItemsOf(parentId: string): PmItem[] {
  const items: PmItem[] = [];
  const folders: Collection[] = collectionStore.collections
    .filter((c) => c.parentId === parentId && c.kind === "folder")
    .sort((a, b) => a.sortOrder - b.sortOrder || a.name.localeCompare(b.name));
  for (const f of folders) {
    items.push({ name: f.name, item: exportItemsOf(f.id) });
  }
  const reqs: RequestItem[] = collectionStore.requestsOf(parentId);
  for (const r of reqs) items.push(requestItemToPmItem(r));
  return items;
}

function close() {
  emit("update:show", false);
}

// 导入 Postman
async function onImport() {
  importMsg.value = null;
  try {
    const filePath = await openDialog({
      multiple: false,
      filters: [{ name: "Postman Collection", extensions: ["json"] }],
    });
    if (!filePath) return;
    importing.value = true;
    const text = await invoke<string>("read_text_file", { path: filePath });
    const json = JSON.parse(text);
    const tree = parsePostmanCollection(json);
    const count = await collectionStore.importTree(tree);
    importMsg.value = { ok: true, text: `成功导入 ${count} 个请求` };
  } catch (e: any) {
    importMsg.value = { ok: false, text: `导入失败：${String(e)}` };
  } finally {
    importing.value = false;
  }
}

// 导入 OpenAPI / Swagger
async function onImportOpenApi() {
  oaMsg.value = null;
  try {
    const filePath = await openDialog({
      multiple: false,
      filters: [
        { name: "OpenAPI / Swagger", extensions: ["json", "yaml", "yml"] },
      ],
    });
    if (!filePath) return;
    oaImporting.value = true;
    const text = await invoke<string>("read_text_file", { path: filePath });
    const { tree, count } = parseOpenApiText(text);
    await collectionStore.importTree(tree);
    oaMsg.value = {
      ok: true,
      text: `成功导入「${tree.name}」共 ${count} 个请求（按 tag 分组）`,
    };
  } catch (e: any) {
    oaMsg.value = { ok: false, text: `导入失败：${String(e)}` };
  } finally {
    oaImporting.value = false;
  }
}

// 导出
async function onExport() {
  exportMsg.value = null;
  if (!exportColId.value) {
    exportMsg.value = { ok: false, text: "请选择要导出的集合" };
    return;
  }
  const col = collectionStore.collections.find((c) => c.id === exportColId.value);
  if (!col) return;
  try {
    exporting.value = true;
    // 按树导出（保留文件夹层级），而不是把请求拉平成顶层 item
    const items = exportItemsOf(col.id);
    const pm = buildPostmanCollectionTree(col.name, items);
    const jsonStr = JSON.stringify(pm, null, 2);
    const outPath = await saveDialog({
      defaultPath: `${col.name.replace(/\s+/g, "-").toLowerCase()}.postman_collection.json`,
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (!outPath) return;
    await invoke("write_text_file", { path: outPath, content: jsonStr });
    exportMsg.value = { ok: true, text: `已导出 ${countRequests(items)} 个请求到文件` };
  } catch (e: any) {
    exportMsg.value = { ok: false, text: `导出失败：${String(e)}` };
  } finally {
    exporting.value = false;
  }
}
</script>

<template>
  <TransitionRoot appear :show="show" as="template">
    <Dialog as="div" class="relative z-50" @close="close">
      <TransitionChild
        as="template"
        enter="duration-200 ease-out"
        enter-from="opacity-0"
        enter-to="opacity-100"
        leave="duration-150 ease-in"
        leave-from="opacity-100"
        leave-to="opacity-0"
      >
        <div class="fixed inset-0 bg-black/50" />
      </TransitionChild>

      <div class="fixed inset-0 flex items-center justify-center p-4">
        <TransitionChild
          as="template"
          enter="duration-200 ease-out"
          enter-from="opacity-0 scale-95"
          enter-to="opacity-100 scale-100"
          leave="duration-150 ease-in"
          leave-from="opacity-100 scale-100"
          leave-to="opacity-0 scale-95"
        >
          <DialogPanel
            class="w-[560px] max-w-full bg-app-surface2 border border-app-border rounded-lg shadow-2xl flex flex-col overflow-hidden"
          >
            <DialogTitle class="px-4 h-10 flex items-center border-b border-app-border text-sm font-medium">
              导入 / 导出
              <button class="ml-auto btn-ghost btn-sm" @click="close">关闭</button>
            </DialogTitle>

            <!-- 格式切换 -->
            <div class="flex gap-1 px-4 pt-3">
              <button class="seg" :class="tab === 'postman' ? 'seg-active' : ''" @click="tab = 'postman'">
                Postman
              </button>
              <button class="seg" :class="tab === 'openapi' ? 'seg-active' : ''" @click="tab = 'openapi'">
                OpenAPI / Swagger
              </button>
            </div>

            <div class="p-4 pt-3 flex flex-col gap-4">
              <template v-if="tab === 'postman'">
                <!-- 导入 Postman -->
                <section class="border border-app-border rounded-md p-3">
                  <div class="flex items-center gap-2 mb-2">
                    <Upload :size="16" class="text-accent-green" />
                    <span class="font-medium text-sm">导入 Postman Collection</span>
                  </div>
                  <p class="text-xs text-app-muted mb-2">
                    选择 Postman v2.1 JSON 文件，将作为新集合导入（保留文件夹结构和脚本）。
                  </p>
                  <button class="btn-primary btn-sm" :disabled="importing" @click="onImport">
                    选择文件导入
                  </button>
                  <div
                    v-if="importMsg"
                    class="mt-2 text-xs flex items-center gap-1"
                    :class="importMsg.ok ? 'text-accent-green' : 'text-accent-red'"
                  >
                    <Check v-if="importMsg.ok" :size="12" />
                    {{ importMsg.text }}
                  </div>
                </section>

                <!-- 导出 Postman -->
                <section class="border border-app-border rounded-md p-3">
                  <div class="flex items-center gap-2 mb-2">
                    <Download :size="16" class="text-accent-blue" />
                    <span class="font-medium text-sm">导出为 Postman Collection</span>
                  </div>
                  <p class="text-xs text-app-muted mb-2">
                    选择一个集合，导出为 v2.1 JSON（包含请求、Body、Headers 和测试脚本）。
                  </p>
                  <div class="flex items-center gap-2">
                    <select v-model="exportColId" class="input flex-1">
                      <option value="" disabled>选择集合...</option>
                      <option v-for="c in collections" :key="c.id" :value="c.id">
                        {{ c.name }}
                      </option>
                    </select>
                    <button class="btn-primary btn-sm" :disabled="exporting" @click="onExport">
                      导出
                    </button>
                  </div>
                  <div
                    v-if="exportMsg"
                    class="mt-2 text-xs flex items-center gap-1"
                    :class="exportMsg.ok ? 'text-accent-green' : 'text-accent-red'"
                  >
                    <Check v-if="exportMsg.ok" :size="12" />
                    {{ exportMsg.text }}
                  </div>
                </section>
              </template>

              <!-- OpenAPI -->
              <template v-else>
                <section class="border border-app-border rounded-md p-3">
                  <div class="flex items-center gap-2 mb-2">
                    <FileJson :size="16" class="text-accent-green" />
                    <span class="font-medium text-sm">导入 OpenAPI / Swagger 文档</span>
                  </div>
                  <p class="text-xs text-app-muted mb-2">
                    选择 openapi.json / openapi.yaml（v3）或 swagger.json（v2）文件。
                    以 info.title 作为集合名，按 tag 分组成文件夹；URL 使用 servers[0] 拼成绝对地址，
                    Body 依据 schema 自动生成示例 JSON，可直接发送调试。
                  </p>
                  <button class="btn-primary btn-sm" :disabled="oaImporting" @click="onImportOpenApi">
                    选择文件导入
                  </button>
                  <div
                    v-if="oaMsg"
                    class="mt-2 text-xs flex items-center gap-1"
                    :class="oaMsg.ok ? 'text-accent-green' : 'text-accent-red'"
                  >
                    <Check v-if="oaMsg.ok" :size="12" />
                    {{ oaMsg.text }}
                  </div>
                </section>
              </template>
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

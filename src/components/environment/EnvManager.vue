<script setup lang="ts">
import { ref, watch, computed } from "vue";
import {
  TransitionRoot,
  TransitionChild,
  Dialog,
  DialogPanel,
  DialogTitle,
} from "@headlessui/vue";
import { Plus, Trash2, Copy, Settings2 } from "lucide-vue-next";
import { useEnvironmentStore } from "@/stores/environment";
import { useToast } from "@/composables/useToast";
import type { Environment, KeyValue } from "@/types";
import KeyValueEditor from "@/components/common/KeyValueEditor.vue";
import EmptyState from "@/components/common/EmptyState.vue";

const props = defineProps<{ show: boolean }>();
const emit = defineEmits<{ (e: "update:show", v: boolean): void }>();

const envStore = useEnvironmentStore();
const toast = useToast();

// 选中的环境 id
const selectedId = ref<string | null>(null);
const selectedEnv = computed<Environment | undefined>(() =>
  envStore.environments.find((e) => e.id === selectedId.value)
);

// 本地编辑副本：name + variables
const editingName = ref("");
const editingVars = ref<KeyValue[]>([]);

watch(
  () => props.show,
  (show) => {
    if (show && !selectedId.value && envStore.environments.length) {
      // 优先选中当前激活环境（用户最常想改的），其次第一个
      const target = envStore.active ?? envStore.environments[0];
      if (target) select(target.id);
    }
  }
);

/** 把当前编辑内容落库；失败提示而不是静默丢弃 */
async function persist() {
  if (!selectedId.value) return;
  // 立即捕获当前值，避免切换环境后被覆盖
  const id = selectedId.value;
  const name = editingName.value;
  const vars = editingVars.value.filter((v) => v.key);
  try {
    await envStore.save({ id, name, variables: vars });
  } catch (e) {
    toast.push(`环境保存失败：${String(e)}`, "error", 5000);
  }
}

/** 切换环境前先保存当前编辑内容 */
function select(id: string) {
  if (selectedId.value && id !== selectedId.value) void persist();
  selectedId.value = id;
  const env = envStore.environments.find((e) => e.id === id);
  if (env) {
    editingName.value = env.name;
    editingVars.value = envStore.parseVariables(env).map((v) => ({ ...v }));
  }
}

function onVarsChange(v: KeyValue[]) {
  editingVars.value = v;
}

async function saveCurrent() {
  if (!selectedId.value) return;
  await envStore.save({
    id: selectedId.value,
    name: editingName.value,
    variables: editingVars.value.filter((v) => v.key),
  });
}

async function createNew() {
  await persist(); // 先保存当前环境编辑
  const env = await envStore.save({ name: "New Environment", variables: [] });
  selectedId.value = env.id;
  editingName.value = env.name;
  editingVars.value = [];
}

async function removeCurrent() {
  if (!selectedId.value) return;
  if (!window.confirm("删除该环境？")) return;
  await envStore.remove(selectedId.value);
  selectedId.value = envStore.environments[0]?.id ?? null;
  if (selectedId.value) select(selectedId.value);
}

async function close() {
  await persist(); // 关闭前保存当前编辑（失败会 toast 提示）
  emit("update:show", false);
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
            class="w-[800px] max-w-full h-[520px] bg-app-surface2 border border-app-border rounded-md shadow-2xl flex flex-col overflow-hidden"
          >
            <DialogTitle class="px-4 py-3 border-b border-app-border text-sm font-medium flex items-center">
              管理环境变量
              <button class="ml-auto btn-ghost btn-sm" @click="close">关闭</button>
            </DialogTitle>

            <div class="flex-1 flex min-h-0">
              <!-- 左：环境列表 -->
              <div class="w-52 border-r border-app-border flex flex-col bg-app-surface/60">
                <div class="p-2">
                  <button class="btn-ghost btn-sm w-full justify-center" @click="createNew">
                    <Plus :size="14" /> 新建环境
                  </button>
                </div>
                <div class="flex-1 overflow-auto">
                  <button
                    v-for="env in envStore.environments"
                    :key="env.id"
                    class="w-full flex items-center gap-2 px-3 py-2 text-sm text-left hover:bg-app-hover rounded-md"
                    :class="env.id === selectedId ? 'bg-app-hover' : ''"
                    @click="select(env.id)"
                  >
                    <span
                      class="w-1.5 h-1.5 rounded-full flex-shrink-0"
                      :class="env.isActive ? 'bg-accent-green' : 'bg-transparent'"
                    />
                    <span class="truncate">{{ env.name }}</span>
                  </button>
                </div>
              </div>

              <!-- 右：编辑区 -->
              <div class="flex-1 flex flex-col min-h-0" v-if="selectedEnv">
                <div class="flex items-center gap-2 px-3 py-2 border-b border-app-border">
                  <input
                    v-model="editingName"
                    class="input flex-1"
                    placeholder="环境名称"
                  />
                  <button class="btn-primary btn-sm" @click="saveCurrent">保存</button>
                  <button class="btn-danger btn-sm" @click="removeCurrent">
                    <Trash2 :size="14" />
                  </button>
                </div>
                <div class="px-3 py-1.5 text-xs text-app-muted border-b border-app-border/40">
                  变量（在 URL / Header / Body 中使用 `{{ '<key>' }}` 引用）
                </div>
                <div class="flex-1 min-h-0">
                  <KeyValueEditor
                    :model-value="editingVars"
                    key-placeholder="变量名（如 baseUrl）"
                    value-placeholder="值"
                    @update:model-value="onVarsChange"
                  />
                </div>
              </div>
              <EmptyState v-else :icon="Settings2" title="选择或新建一个环境" />
            </div>
          </DialogPanel>
        </TransitionChild>
      </div>
    </Dialog>
  </TransitionRoot>
</template>

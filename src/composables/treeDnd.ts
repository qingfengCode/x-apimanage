import { ref } from "vue";

/**
 * 树拖拽共享状态：TreeNode 拖拽源设置，CollectionTree 的"顶层放置条"读取。
 * 拖拽结束（dragend，浏览器保证触发）时清除。
 */
export const treeDragNodeId = ref<string | null>(null);

export function clearTreeDrag() {
  treeDragNodeId.value = null;
}

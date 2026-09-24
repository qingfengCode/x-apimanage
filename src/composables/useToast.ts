import { ref } from "vue";

export type ToastType = "success" | "error" | "info";

export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
  action?: ToastAction;
}

const toasts = ref<ToastItem[]>([]);
let nextId = 1;

function push(
  message: string,
  type: ToastType = "info",
  duration = 3200,
  action?: ToastAction
): number {
  const id = nextId++;
  toasts.value.push({ id, type, message, action });
  setTimeout(() => dismiss(id), duration);
  return id;
}

function dismiss(id: number) {
  toasts.value = toasts.value.filter((t) => t.id !== id);
}

/**
 * 轻量全局 Toast：模块级响应式状态，任何组件可直接调用。
 * const { push } = useToast(); push("已保存", "success");
 */
export function useToast() {
  return { toasts, push, dismiss };
}

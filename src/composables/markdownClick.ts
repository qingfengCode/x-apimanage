import type { Ref } from "vue";
import { copyText } from "@/utils/curl";
import { useToast } from "@/composables/useToast";

/**
 * 给渲染了 Markdown 的容器挂"点击代码块整段复制"：
 * 点击 <pre> 内部且当前无文本选区时，复制整段代码并 toast。
 * 返回的 handler 绑定到容器 click 事件。
 */
export function makeCodeCopyHandler(container: Ref<HTMLElement | null>) {
  return async (e: MouseEvent) => {
    const el = container.value;
    if (!el) return;
    const target = e.target as HTMLElement;
    if (!el.contains(target)) return;
    const pre = target.closest("pre");
    if (!pre) return;
    // 用户正在选中文本（想复制一部分）时不拦截
    const sel = window.getSelection();
    if (sel && sel.toString().length > 0) return;
    const code = pre.innerText.replace(/\n$/, "");
    if (await copyText(code)) {
      useToast().push("已复制代码块", "success", 1500);
    }
  };
}

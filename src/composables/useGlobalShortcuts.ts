import { onMounted, onBeforeUnmount } from "vue";
import { useTabStore } from "@/stores/tab";
import { useAiStore } from "@/stores/ai";
import { useToast } from "@/composables/useToast";

/**
 * 全局快捷键体系
 * - Ctrl/Cmd + T     新建请求 Tab
 * - Ctrl/Cmd + W     关闭当前 Tab
 * - Ctrl/Cmd + S     保存当前请求
 * - Ctrl/Cmd + Enter 发送当前请求
 * - Ctrl/Cmd + Shift + T  恢复最近关闭的 Tab
 * - Ctrl/Cmd + Tab   下一个 Tab
 * - Ctrl/Cmd + Shift + Tab  上一个 Tab
 * - Ctrl/Cmd + I     切换 AI 助手面板
 * - Ctrl/Cmd + /     快捷键帮助
 *
 * 发送/保存通过自定义事件向外广播，由 RequestPanel 监听处理，
 * 避免快捷键层直接耦合业务逻辑。
 */
export function useGlobalShortcuts() {
  const tabStore = useTabStore();
  const aiStore = useAiStore();

  function isCtrl(e: KeyboardEvent): boolean {
    return e.ctrlKey || e.metaKey;
  }

  function inEditableTarget(t: EventTarget | null): boolean {
    const el = t as HTMLElement | null;
    if (!el) return false;
    const tag = el.tagName;
    return (
      tag === "INPUT" ||
      tag === "TEXTAREA" ||
      tag === "SELECT" ||
      el.isContentEditable
    );
  }

  function onKeydown(e: KeyboardEvent) {
    // Ctrl+W 在浏览器默认会关闭页面，必须 preventDefault
    if (isCtrl(e) && (e.key === "w" || e.key === "W")) {
      e.preventDefault();
      if (tabStore.activeTabId) tabStore.closeTabWithConfirm(tabStore.activeTabId);
      return;
    }
    // Ctrl+Shift+T 恢复最近关闭的 Tab
    if (isCtrl(e) && e.shiftKey && (e.key === "t" || e.key === "T")) {
      e.preventDefault();
      if (!tabStore.restoreLastClosed()) {
        useToast().push("没有可恢复的已关闭 Tab", "info", 2000);
      }
      return;
    }
    // Ctrl+T 浏览器默认开新标签，必须 preventDefault
    if (isCtrl(e) && !e.shiftKey && (e.key === "t" || e.key === "T")) {
      // 在编辑框内不触发，避免影响输入
      if (inEditableTarget(e.target)) return;
      e.preventDefault();
      tabStore.newTab();
      return;
    }
    // Ctrl+S 保存（拦截默认"另存网页"）
    if (isCtrl(e) && (e.key === "s" || e.key === "S")) {
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("shortcut:save"));
      return;
    }
    // Ctrl+Enter 发送；在可编辑区域（输入框/Monaco/AI 输入等）内不抢，
    // 由各编辑器自行处理（如 URL 输入框有本地 Ctrl+Enter 发送、AI 输入框发 AI 消息）
    if (isCtrl(e) && e.key === "Enter") {
      if (inEditableTarget(e.target)) return;
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("shortcut:send"));
      return;
    }
    // Ctrl+/ 快捷键帮助（编辑器中是注释功能，故跳过可编辑目标）
    if (isCtrl(e) && e.key === "/") {
      if (inEditableTarget(e.target)) return;
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("shortcut:help"));
      return;
    }
    // Ctrl+I 切换 AI 助手面板（Monaco 里 Ctrl+I 无默认冲突）
    if (isCtrl(e) && !e.shiftKey && (e.key === "i" || e.key === "I")) {
      e.preventDefault();
      aiStore.togglePanel();
      return;
    }
    // Ctrl+P 命令面板（浏览器打印用得少，直接接管；Monaco 内 Ctrl+P 是查找，
    // 编辑区内不抢）
    if (isCtrl(e) && (e.key === "p" || e.key === "P")) {
      if (inEditableTarget(e.target)) return;
      e.preventDefault();
      window.dispatchEvent(new CustomEvent("shortcut:palette"));
      return;
    }
    // Ctrl+Tab / Ctrl+Shift+Tab 切换 Tab
    if (isCtrl(e) && e.key === "Tab") {
      e.preventDefault();
      const tabs = tabStore.tabs;
      if (tabs.length < 2) return;
      const idx = tabs.findIndex((t) => t.id === tabStore.activeTabId);
      const next = e.shiftKey
        ? (idx - 1 + tabs.length) % tabs.length
        : (idx + 1) % tabs.length;
      tabStore.setActive(tabs[next].id);
      return;
    }
  }

  onMounted(() => window.addEventListener("keydown", onKeydown, true));
  onBeforeUnmount(() => window.removeEventListener("keydown", onKeydown, true));
}

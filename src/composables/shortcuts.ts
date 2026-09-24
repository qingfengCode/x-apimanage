/**
 * 全局快捷键注册表（显示与分发共用同一份定义，避免描述与实现脱节）。
 */
export interface ShortcutDef {
  keys: string[];
  label: string;
  section: "请求" | "窗口" | "AI";
}

export const SHORTCUTS: ShortcutDef[] = [
  { keys: ["Ctrl/Cmd+T"], label: "新建请求 Tab", section: "窗口" },
  { keys: ["Ctrl/Cmd+W"], label: "关闭当前 Tab", section: "窗口" },
  { keys: ["Ctrl/Cmd+Shift+T"], label: "恢复最近关闭的 Tab", section: "窗口" },
  { keys: ["Ctrl/Cmd+S"], label: "保存请求到集合", section: "请求" },
  { keys: ["Ctrl/Cmd+Enter"], label: "发送当前请求", section: "请求" },
  { keys: ["Enter"], label: "（URL 栏内）直接发送", section: "请求" },
  { keys: ["Ctrl/Cmd+I"], label: "打开/关闭 AI 助手", section: "AI" },
  { keys: ["Ctrl/Cmd+/"], label: "快捷键帮助", section: "窗口" },
  { keys: ["Ctrl/Cmd+Tab"], label: "下一个 Tab", section: "窗口" },
  { keys: ["Ctrl/Cmd+Shift+Tab"], label: "上一个 Tab", section: "窗口" },
];

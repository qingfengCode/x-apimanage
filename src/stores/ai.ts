import { invoke, Channel } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type {
  AiChatEvent,
  AiChatItem,
  AiSessionMeta,
  AiSettings,
  McpServerInfo,
} from "@/types";
import { useToast } from "@/composables/useToast";
import { uid } from "@/utils/id";

interface AiState {
  settings: AiSettings;
  settingsLoaded: boolean;
  /** 右侧聊天面板是否可见 */
  panelVisible: boolean;
  /** MCP 面板是否可见 */
  mcpPanelVisible: boolean;
  /** MCP 服务详情（面板展示用） */
  mcpInfo: McpServerInfo | null;
  /** 面板宽度（px，可拖拽，持久化） */
  panelWidth: number;
  /** 历史会话列表 */
  sessions: AiSessionMeta[];
  /** 当前会话 id（null = 新会话尚未保存） */
  currentSessionId: string | null;
  /** 当前会话标题（新建后由首条消息生成） */
  currentTitle: string;
  /** 聊天消息条目（界面渲染用） */
  items: AiChatItem[];
  /** 发送中 */
  busy: boolean;
  /**
   * 请求代际：每次 stop()/send() 自增。
   * 在途请求只认自己发起时的 gen，代际变化后其流式事件与收尾全部失效，
   * 避免旧请求的 channel 事件污染新一轮对话。
   */
  gen: number;
  /** 最近一次发送的 requestId（传给后端用于真实取消在途流） */
  lastRequestId: string | null;
  /** 当前状态提示（"正在思考…" 等） */
  status: string;
  /** 底层会话历史（user/assistant 文本对，传回后端保持上下文） */
  session: { role: "user" | "assistant"; content: string }[];
  /** 是否把当前请求作为上下文附带 */
  attachContext: boolean;
  /** MCP 服务运行地址（null = 未启动），仅作状态展示 */
  mcpUrl: string | null;
}

const emptySettings: AiSettings = {
  baseUrl: "",
  apiKey: "",
  model: "",
  systemPrompt: "",
  mcpEnabled: false,
  mcpPort: 8765,
  mcpToken: "",
};

const LS_CHAT_KEY = "x-apimanage.ai-chat";
const LS_WIDTH_KEY = "x-apimanage.ai-width";

/** 把底层错误映射为可行动的提示 */
function friendlyAiError(e: unknown): string {
  const s = String(e);
  if (s.includes("401") || s.toLowerCase().includes("unauthorized"))
    return "认证失败：API Key 无效或过期，请检查设置";
  if (s.includes("404"))
    return "接口不存在：Base URL 可能不正确（通常以 /v1 结尾），或模型名写错";
  if (s.includes("429"))
    return "请求过于频繁或余额不足（429），请稍后重试";
  if (s.includes("dns error") || s.includes("failed to lookup address"))
    return "无法连接 AI 服务：域名解析失败，请检查网络或 Base URL";
  if (s.includes("Connection refused") || s.includes("connect error"))
    return "无法连接 AI 服务：服务未启动或地址/端口错误（本地 Ollama 请确认已运行）";
  if (s.includes("operation timed out") || s.includes("Timeout"))
    return "AI 服务响应超时，请稍后重试或换用更快的模型";
  return s;
}

/** 旧版 localStorage 聊天迁移（一次性） */
function loadLegacyChat(): { items: AiChatItem[]; session: AiState["session"] } | null {
  try {
    const raw = localStorage.getItem(LS_CHAT_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (
      Array.isArray(parsed?.items) &&
      parsed.items.length &&
      Array.isArray(parsed?.session)
    ) {
      return { items: parsed.items, session: parsed.session };
    }
  } catch {
    /* ignore */
  }
  return null;
}

export const useAiStore = defineStore("ai", {
  state: (): AiState => ({
    settings: { ...emptySettings },
    settingsLoaded: false,
    panelVisible: false,
    mcpPanelVisible: false,
    mcpInfo: null,
    panelWidth: Number(localStorage.getItem(LS_WIDTH_KEY)) || 380,
    sessions: [],
    currentSessionId: null,
    currentTitle: "",
    items: [],
    busy: false,
    gen: 0,
    lastRequestId: null,
    status: "",
    session: [],
    attachContext: true,
    mcpUrl: null,
  }),

  actions: {
    setPanelWidth(w: number) {
      this.panelWidth = Math.min(Math.max(w, 300), 640);
      localStorage.setItem(LS_WIDTH_KEY, String(this.panelWidth));
    },

    async loadSettings() {
      this.settings = await invoke<AiSettings>("get_ai_settings");
      this.settingsLoaded = true;
    },

    async saveSettings(s: AiSettings) {
      await invoke("save_ai_settings", { settings: s });
      this.settings = { ...s };
    },

    async testConnection(): Promise<string> {
      return invoke<string>("test_ai_connection");
    },

    async refreshMcpStatus() {
      try {
        this.mcpUrl = await invoke<string | null>("mcp_server_status");
      } catch {
        this.mcpUrl = null;
      }
    },

    // ---- MCP 面板 ----

    toggleMcpPanel(v?: boolean) {
      this.mcpPanelVisible = v ?? !this.mcpPanelVisible;
      if (this.mcpPanelVisible) this.refreshMcpInfo();
    },

    async refreshMcpInfo() {
      try {
        this.mcpInfo = await invoke<McpServerInfo>("mcp_server_info");
        this.mcpUrl = this.mcpInfo.url;
        this.settings.mcpToken = this.mcpInfo.token;
        this.settings.mcpPort = this.mcpInfo.port;
      } catch {
        this.mcpInfo = null;
      }
    },

    /** 生成新的访问密钥（服务运行中会自动重启切换为局域网+鉴权模式） */
    async generateToken(): Promise<string> {
      const token = await invoke<string>("generate_mcp_token");
      this.settings.mcpToken = token;
      await this.refreshMcpInfo();
      return token;
    },

    /** 吊销密钥（恢复仅本机回环） */
    async revokeToken() {
      await invoke("revoke_mcp_token");
      this.settings.mcpToken = "";
      await this.refreshMcpInfo();
    },

    /** 面板里复制的客户端连接配置（mcpServers JSON 片段） */
    mcpConfigJson(): string {
      const info = this.mcpInfo;
      const host = info?.token ? info.lanIp : "127.0.0.1";
      const port = info?.port ?? this.settings.mcpPort;
      const server: Record<string, unknown> = {
        url: `http://${host}:${port}/mcp`,
      };
      if (info?.token) {
        server.headers = { Authorization: `Bearer ${info.token}` };
      }
      return JSON.stringify({ mcpServers: { "x-apimanage": server } }, null, 2);
    },

    /** 启动 / 停止 MCP 服务（返回启动后的 URL，null = 已停止） */
    async setMcp(enabled: boolean): Promise<string | null> {
      const url = await invoke<string | null>("set_mcp_server", {
        enabled,
        port: this.settings.mcpPort,
      });
      this.mcpUrl = url;
      this.settings.mcpEnabled = !!url;
      return url;
    },

    /** 就地改端口：先存设置，运行中则自动重启 */
    async changeMcpPort(port: number) {
      this.settings.mcpPort = port;
      if (this.mcpUrl) {
        // set_mcp_server 内部会先停旧实例再按新端口启动
        this.mcpUrl = await invoke<string | null>("set_mcp_server", {
          enabled: true,
          port,
        });
      } else {
        // 未运行也把端口持久化，下次启动生效
        await this.saveSettings({ ...this.settings });
      }
    },

    togglePanel(v?: boolean) {
      this.panelVisible = v ?? !this.panelVisible;
      if (this.panelVisible) this.refreshMcpStatus();
    },

    // ---- 会话管理 ----

    /** 拉取会话列表；若无任何会话且 localStorage 有旧聊天则迁移 */
    async loadSessions() {
      this.sessions = await invoke<AiSessionMeta[]>("list_ai_sessions");
      if (!this.sessions.length) {
        const legacy = loadLegacyChat();
        if (legacy) {
          await this.restoreInto(legacy.items, legacy.session, "导入的会话");
          localStorage.removeItem(LS_CHAT_KEY);
        }
      }
    },

    /** 把一组内容落成当前会话（不切换 UI 状态之外的任何东西） */
    async restoreInto(
      items: AiChatItem[],
      session: AiState["session"],
      title: string
    ) {
      const meta = await invoke<AiSessionMeta>("save_ai_session", {
        input: {
          title,
          items: JSON.stringify(items),
          history: JSON.stringify(session),
        },
      });
      this.sessions.unshift(meta);
      this.currentSessionId = meta.id;
      this.currentTitle = meta.title;
      this.items = items;
      this.session = session;
    },

    async selectSession(id: string) {
      if (this.busy) return;
      const full = await invoke<{ id: string; title: string; items: string; history: string }>(
        "get_ai_session",
        { id }
      );
      this.currentSessionId = full.id;
      this.currentTitle = full.title;
      try {
        this.items = JSON.parse(full.items);
        this.session = JSON.parse(full.history);
      } catch {
        this.items = [];
        this.session = [];
      }
    },

    newSession() {
      if (this.busy) return;
      this.currentSessionId = null;
      this.currentTitle = "";
      this.items = [];
      this.session = [];
      this.status = "";
    },

    async deleteSession(id: string) {
      // 删除正在生成的会话：先停止（作废旧请求代际），否则结束时的
      // persistSession 会把刚删除的会话重新 upsert 回来
      if (this.busy && this.currentSessionId === id) this.stop();
      await invoke("delete_ai_session", { id });
      this.sessions = this.sessions.filter((s) => s.id !== id);
      if (this.currentSessionId === id) {
        this.currentSessionId = null;
        this.currentTitle = "";
        this.items = [];
        this.session = [];
        this.status = "";
      }
    },

    /** 持久化当前会话（发送完成后调用） */
    async persistSession() {
      if (!this.items.length) return;
      const firstUser = this.items.find((i) => i.kind === "user");
      const title =
        this.currentTitle || (firstUser ? firstUser.text.slice(0, 24) : "新会话");
      try {
        const meta = await invoke<AiSessionMeta>("save_ai_session", {
          input: {
            id: this.currentSessionId ?? undefined,
            title,
            items: JSON.stringify(this.items.slice(-80)),
            history: JSON.stringify(this.session.slice(-40)),
          },
        });
        this.currentSessionId = meta.id;
        this.currentTitle = meta.title;
        const idx = this.sessions.findIndex((s) => s.id === meta.id);
        if (idx >= 0) this.sessions.splice(idx, 1, meta);
        else this.sessions.unshift(meta);
        // 按更新时间重排（当前会话置顶）
        this.sessions.sort((a, b) => b.updatedAt - a.updatedAt);
      } catch {
        /* 保存失败不阻断聊天 */
      }
    },

    clearChat() {
      this.items = [];
      this.session = [];
      this.status = "";
      this.persistSession();
    },

    /** 用户主动停止：作废在途请求的代际并解除 busy，并真实取消后端流 */
    stop() {
      if (!this.busy) return;
      const id = this.lastRequestId;
      this.gen++;
      this.busy = false;
      this.status = "";
      this.items.push({ kind: "status", text: "已停止生成" });
      this.lastRequestId = null;
      // 通知后端停止拉取/继续生成（失败无碍，代际已保证事件不会串扰）
      if (id) invoke("cancel_ai_chat", { requestId: id }).catch(() => {});
      this.persistSession();
    },

    /** 发送一条消息（context 为当前请求上下文文本，可为 null） */
    async send(text: string, context: string | null) {
      const content = text.trim();
      if (!content || this.busy) return;

      this.items.push({ kind: "user", text: content });
      this.session.push({ role: "user", content });
      // 开启新代际：任何更早的在途请求（含被 stop 的）都不再生效
      this.gen++;
      const myGen = this.gen;
      const requestId = uid("ai-req");
      this.lastRequestId = requestId;
      this.busy = true;
      this.status = "正在思考…";

      const toast = useToast();

      const channel = new Channel<AiChatEvent>();
      channel.onmessage = (ev) => {
        if (this.gen !== myGen) return;
        if (ev.type === "status") {
          this.status = ev.text;
        } else if (ev.type === "delta") {
          // 打字机：追加到最后一条 assistant 消息（无则创建）
          const last = this.items[this.items.length - 1];
          if (last && last.kind === "assistant") {
            last.text += ev.text;
            last.streaming = true;
          } else {
            this.items.push({ kind: "assistant", text: ev.text, streaming: true });
          }
        } else if (ev.type === "tool") {
          this.items.push({ kind: "tool", text: ev.result, toolName: ev.name, ok: ev.ok });
          // 关键工具成功时给即时反馈
          if (ev.ok && ev.name === "create_request")
            toast.push("AI 已创建请求，见左侧集合", "success");
          if (ev.ok && ev.name === "save_document")
            toast.push("AI 已保存文档，见左侧 Docs 页签", "success");
        } else if (ev.type === "done") {
          // 以服务端完整内容为准，校正最后一条 assistant 消息并结束打字机状态
          const last = this.items[this.items.length - 1];
          if (last && last.kind === "assistant") {
            last.text = ev.content;
            last.streaming = false;
          } else {
            this.items.push({ kind: "assistant", text: ev.content });
          }
          this.session.push({ role: "assistant", content: ev.content });
        } else if (ev.type === "error") {
          this.items.push({ kind: "error", text: friendlyAiError(ev.message) });
        }
      };

      try {
        await invoke("ai_chat", {
          requestId,
          messages: this.session, // 完整历史（尾部即刚 push 的当前用户消息）
          context: this.attachContext ? context : null,
          onEvent: channel,
        });
      } catch (e) {
        if (this.gen === myGen) {
          this.items.push({ kind: "error", text: friendlyAiError(e) });
        }
      } finally {
        // 只允许最新代际收尾，避免被 stop/新消息作废的旧请求改写状态或重复持久化
        if (this.gen === myGen) {
          this.busy = false;
          this.status = "";
          this.lastRequestId = null;
          this.persistSession();
        }
      }
    },
  },
});

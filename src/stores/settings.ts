import { invoke } from "@tauri-apps/api/core";
import { defineStore } from "pinia";
import type { ProxySettings, ProxyTestResult } from "@/types";

interface SettingsState {
  proxy: ProxySettings;
  loaded: boolean;
}

const emptyProxy: ProxySettings = { enabled: false, url: "", bypass: "" };

/** 把后端返回的连接错误压成一句可行动的中文提示 */
function friendlyProxyError(e: unknown): string {
  const raw = String(e).replace(/^HTTP 请求错误:\s*/, "");
  if (/connection refused|tcp connect error/i.test(raw))
    return "连接被拒绝：代理没有在监听，检查代理客户端是否已启动、地址和端口是否正确";
  if (/dns error|failed to lookup address|invalid dns name/i.test(raw))
    return "域名解析失败：代理地址或测试地址的主机名写错了";
  if (/timed out|timeout/i.test(raw))
    return "连接超时：代理无响应，可能是地址/端口错误或被防火墙拦截";
  if (/certificate|tls|handshake/i.test(raw))
    return "TLS 握手失败：HTTPS 代理握手不成功，可试试 http:// 或 socks5:// 协议";
  return raw;
}

export const useSettingsStore = defineStore("settings", {
  state: (): SettingsState => ({
    proxy: { ...emptyProxy },
    loaded: false,
  }),

  actions: {
    async load() {
      this.proxy = await invoke<ProxySettings>("get_proxy_settings");
      this.loaded = true;
    },

    /** 保存并立即生效（后端会重建所有出站请求共用的客户端） */
    async saveProxy(proxy: ProxySettings) {
      await invoke("save_proxy_settings", { settings: proxy });
      this.proxy = { ...proxy };
      this.loaded = true;
    },

    /**
     * 用「待保存」的配置试发一个请求，通过抛错/返回区分成功失败。
     * 失败信息已转成可读文案。
     */
    async testProxy(proxy: ProxySettings, url: string): Promise<ProxyTestResult> {
      try {
        return await invoke<ProxyTestResult>("test_proxy", { settings: proxy, url });
      } catch (e) {
        throw new Error(friendlyProxyError(e));
      }
    },
  },
});

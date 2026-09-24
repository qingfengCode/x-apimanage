import type {
  AuthConfig,
  HttpRequestPayload,
  KeyValue,
  RequestBody,
} from "@/types";
import { useVariable } from "@/composables/useVariable";

// useVariable 不依赖 store/生命周期，可在模块内安全使用
const { resolveString } = useVariable();

/**
 * 请求发送的公共构造逻辑：RequestPanel（单发）与 Collection Runner（批量）共用。
 * 输入统一为「发送时刻快照」，与编辑中的 draft 解耦。
 */

export interface RequestSnapshotLike {
  method: string;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  body: RequestBody | null;
  auth?: AuthConfig | null;
  timeoutMs: number;
}

/** 应用变量解析后构造 payload */
export function buildPayload(
  vars: Map<string, string>,
  src: RequestSnapshotLike
): HttpRequestPayload {
  const url = resolveString(src.url, vars);
  // 去重：ParamsTab 会把参数行同步写回 URL（draft.url 已含 query）。
  // 若 URL 本身已带 query，就不再把 params 行交给引擎追加，否则每个参数
  // 都会被发送两次（?a=1&a=1）；仅当 URL 无 query（如从 Postman 导入的
  // 干净 URL）时由引擎追加 params 行。
  const params = url.includes("?")
    ? []
    : src.params
        .filter((p) => p.enabled && p.key)
        .map((p) => ({ ...p, key: resolveString(p.key, vars), value: resolveString(p.value, vars) }));
  const payload: HttpRequestPayload = {
    method: src.method,
    url,
    timeoutMs: src.timeoutMs,
    params,
    headers: src.headers
      .filter((h) => h.enabled && h.key)
      .map((h) => ({ ...h, key: resolveString(h.key, vars), value: resolveString(h.value, vars) })),
    body: resolveBody(src.body, vars),
  };
  applyAuth(src.auth ?? null, vars, payload.headers, payload.params);
  return payload;
}

/**
 * 把认证配置注入到 headers/params（变量已解析）。
 * 若用户已在 Headers/Params 里手动启用了同名字段则跳过，手动值优先。
 */
export function applyAuth(
  auth: AuthConfig | null,
  vars: Map<string, string>,
  headers: KeyValue[],
  params: KeyValue[]
): void {
  if (!auth) return;
  const hasHeader = (name: string) =>
    headers.some((h) => h.enabled && h.key.toLowerCase() === name.toLowerCase());
  const hasParam = (name: string) => params.some((p) => p.enabled && p.key === name);

  if (auth.type === "bearer") {
    const scheme = resolveString(auth.prefix, vars).trim() || "Bearer";
    const token = resolveString(auth.token, vars);
    if (token && !hasHeader("Authorization")) {
      headers.push({ key: "Authorization", value: `${scheme} ${token}`, enabled: true });
    }
  } else if (auth.type === "basic") {
    const raw = `${resolveString(auth.username, vars)}:${resolveString(auth.password, vars)}`;
    if (!hasHeader("Authorization")) {
      headers.push({ key: "Authorization", value: `Basic ${b64Encode(raw)}`, enabled: true });
    }
  } else if (auth.type === "apikey") {
    const key = resolveString(auth.key, vars);
    const value = resolveString(auth.value, vars);
    if (!key || !value) return;
    if (auth.addTo === "header") {
      if (!hasHeader(key)) headers.push({ key, value, enabled: true });
    } else {
      if (!hasParam(key)) params.push({ key, value, enabled: true });
    }
  }
}

/** UTF-8 安全的 base64（Basic 认证用户名/密码可能含中文） */
export function b64Encode(s: string): string {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
}

export function resolveBody(body: RequestBody | null, vars: Map<string, string>): RequestBody | null {
  if (!body) return null;
  if (body.mode === "raw") {
    return { ...body, raw: resolveString(body.raw, vars) };
  }
  return {
    ...body,
    items: body.items.map((kv) => ({
      ...kv,
      key: resolveString(kv.key, vars),
      // ?? "" 兜底：历史脏数据（如旧版导入的 file 项）可能缺 value
      value: resolveString(kv.value ?? "", vars),
    })),
  };
}

/** 把后端错误翻译成可读提示 */
export function friendlyError(e: any): string {
  const s = String(e);
  if (s.includes("InvalidUrl") || s.includes("invalid url") || s.includes("empty host"))
    return "URL 无效，请检查是否包含协议（如 http://）";
  if (s.includes("dns error") || s.includes("failed to lookup address"))
    return "域名解析失败，请检查网络或域名拼写";
  if (s.includes("connect error") || s.includes("Connection refused"))
    return "连接被拒绝，目标服务可能未启动或端口错误";
  if (s.includes("operation timed out") || s.includes("Timeout"))
    return "请求超时，目标服务响应过慢或不可达";
  if (s.includes("tls") || s.includes("certificate"))
    return "TLS/证书错误，目标服务证书不受信任";
  return s;
}

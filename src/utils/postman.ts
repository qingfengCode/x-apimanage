import type {
  KeyValue,
  RequestBody,
  RequestDraft,
  RequestItem,
} from "@/types";
import type {
  PmBody,
  PmCollection,
  PmEvent,
  PmFormData,
  PmHeader,
  PmItem,
  PmQuery,
  PmRequest,
  PmUrl,
  PmUrlEncoded,
  PmVariable,
} from "@/types/postman";
import { uid } from "@/utils/id";

/** Postman 语言 -> 我们的 mimeType */
function pmLangToMime(lang?: string): string {
  switch ((lang || "text").toLowerCase()) {
    case "json":
      return "application/json";
    case "javascript":
    case "js":
      return "application/javascript";
    case "html":
      return "text/html";
    case "xml":
      return "application/xml";
    default:
      return "text/plain";
  }
}

function mimeToPmLang(mime: string): string {
  const m = mime.toLowerCase();
  if (m.includes("json")) return "json";
  if (m.includes("html")) return "html";
  if (m.includes("xml")) return "xml";
  if (m.includes("javascript")) return "javascript";
  return "text";
}

function urlToPmUrl(raw: string, queries: PmQuery[]): PmUrl {
  // 尽量拆，失败就只填 raw
  try {
    const u = new URL(raw);
    return {
      raw,
      protocol: u.protocol.replace(":", ""),
      host: u.hostname.split("."),
      path: u.pathname.split("/").filter(Boolean),
      query: queries.length ? queries : undefined,
    };
  } catch {
    return { raw, query: queries.length ? queries : undefined };
  }
}

function pmUrlToString(url: PmUrl | string | undefined): string {
  if (!url) return "";
  if (typeof url === "string") return url;
  if (url.raw) return url.raw;
  const proto = url.protocol ? `${url.protocol}://` : "";
  const host = (url.host || []).join(".");
  const path = (url.path || []).join("/");
  let qs = "";
  if (url.query && url.query.length) {
    qs = "?" + url.query.map((q) => `${q.key}=${q.value}`).join("&");
  }
  return `${proto}${host}/${path}${qs}`;
}

/** PmBody -> RequestBody（我们的格式） */
function pmBodyToOurs(body?: PmBody): RequestBody | null {
  if (!body) return null;
  if (body.mode === "raw") {
    return {
      mode: "raw",
      raw: body.raw || "",
      mimeType: pmLangToMime(body.options?.raw?.language),
    };
  }
  if (body.mode === "urlencoded") {
    return {
      mode: "form",
      items: (body.urlencoded || []).map((it: PmUrlEncoded) => ({
        key: it.key,
        value: it.value,
        enabled: !it.disabled,
      })),
    };
  }
  if (body.mode === "formdata") {
    return {
      mode: "multipart",
      items: (body.formdata || []).map((it: PmFormData) => ({
        key: it.key,
        // Postman 的 file 类型条目只有 src 没有 value；本应用只支持文本 part，
        // 降级为文本路径占位，避免 value=undefined 在发送/序列化时崩溃
        value: it.value ?? (it.src ? String(it.src) : ""),
        enabled: !it.disabled,
      })),
    };
  }
  return null;
}

function oursBodyToPm(body: RequestBody | null): PmBody | undefined {
  if (!body) return undefined;
  if (body.mode === "raw") {
    return {
      mode: "raw",
      raw: body.raw,
      options: { raw: { language: mimeToPmLang(body.mimeType) } },
    };
  }
  if (body.mode === "form") {
    return {
      mode: "urlencoded",
      urlencoded: body.items.map((kv) => ({
        key: kv.key,
        value: kv.value,
        disabled: !kv.enabled,
      })),
    };
  }
  return {
    mode: "formdata",
    formdata: body.items.map((kv) => ({
      key: kv.key,
      value: kv.value,
      type: "text",
      disabled: !kv.enabled,
    })),
  };
}

function kvToPmHeaders(headers: KeyValue[]): PmHeader[] {
  return headers.map((h) => ({
    key: h.key,
    value: h.value,
    disabled: !h.enabled,
  }));
}

function pmHeadersToKv(headers?: PmHeader[]): KeyValue[] {
  return (headers || []).map((h) => ({
    key: h.key,
    value: h.value,
    enabled: !h.disabled,
  }));
}

function kvToPmQuery(params: KeyValue[]): PmQuery[] {
  return params.map((p) => ({ key: p.key, value: p.value, disabled: !p.enabled }));
}

function pmQueryToKv(query?: PmQuery[]): KeyValue[] {
  return (query || []).map((q) => ({ key: q.key, value: q.value, enabled: !q.disabled }));
}

function getEvent(events: PmEvent[] | undefined, listen: "prerequest" | "test"): string {
  if (!events) return "";
  const ev = events.find((e) => e.listen === listen);
  if (!ev) return "";
  const exec = ev.script.exec;
  return Array.isArray(exec) ? exec.join("\n") : exec || "";
}

// ============ 导入 ============

export interface ImportedNode {
  type: "folder" | "request";
  name: string;
  // folder
  children?: ImportedNode[];
  // request
  request?: {
    name: string;
    method: string;
    url: string;
    params: KeyValue[];
    headers: KeyValue[];
    body: RequestBody | null;
    preScript: string;
    testScript: string;
  };
}

/** 把 Postman Collection 转换为可导入的树 */
export function parsePostmanCollection(data: unknown): ImportedNode {
  const col = data as PmCollection;
  if (!col || !col.info || !Array.isArray(col.item)) {
    throw new Error("不是合法的 Postman Collection v2.1 文件");
  }
  return {
    type: "folder",
    name: col.info.name || "Imported Collection",
    children: (col.item || []).map(parseItem),
  };
}

function parseItem(item: PmItem): ImportedNode {
  // 文件夹：有 item 子数组
  if (Array.isArray(item.item) && item.item.length > 0) {
    return {
      type: "folder",
      name: item.name || "Folder",
      children: item.item.map(parseItem),
    };
  }
  // 请求
  const req = item.request;
  if (!req) {
    return { type: "folder", name: item.name || "Empty", children: [] };
  }
  let url = pmUrlToString(req.url);
  const params = pmQueryToKv(typeof req.url === "object" && req.url ? req.url.query : undefined);
  // 双重参数去重：Postman 的 url.raw 通常已含 query（?a=1），若再把它拆进 params 表，
  // 发送时后端会把同名参数再 append 一次（?a=1&a=1）。raw 里的 query 只保留一份来源。
  if (params.length) {
    const qIdx = url.indexOf("?");
    if (qIdx >= 0) url = url.slice(0, qIdx);
  }
  const headers = pmHeadersToKv(req.header);
  const body = pmBodyToOurs(req.body);
  return {
    type: "request",
    name: item.name || `${req.method} ${url}`,
    request: {
      name: item.name || `${req.method} ${url}`,
      method: (req.method || "GET").toUpperCase(),
      url,
      params,
      headers,
      body,
      preScript: getEvent(item.event, "prerequest"),
      testScript: getEvent(item.event, "test"),
    },
  };
}

// ============ 导出 ============

/** 把单个请求（草稿或已存）导出为 PmItem */
function oursRequestToPmItem(
  name: string,
  method: string,
  url: string,
  params: KeyValue[],
  headers: KeyValue[],
  body: RequestBody | null,
  preScript: string,
  testScript: string
): PmItem {
  const query = kvToPmQuery(params);
  const item: PmItem = {
    name,
    request: {
      method: method.toUpperCase(),
      header: headers.length ? kvToPmHeaders(headers) : undefined,
      url: urlToPmUrl(url, query),
      body: oursBodyToPm(body),
    },
  };
  const events: PmEvent[] = [];
  if (preScript) {
    events.push({ listen: "prerequest", script: { exec: preScript.split("\n") } });
  }
  if (testScript) {
    events.push({ listen: "test", script: { exec: testScript.split("\n") } });
  }
  if (events.length) item.event = events;
  return item;
}

/** 把一组请求（属于同一集合）导出为 Postman Collection v2.1 JSON 对象 */
export function buildPostmanCollection(
  name: string,
  requests: Array<Pick<RequestItem, "name" | "method" | "url" | "params" | "headers" | "body" | "preScript" | "testScript">>
): PmCollection {
  return {
    info: {
      name,
      schema: "https://schema.postman.com/json/collection/v2.1.0/collection.json",
    },
    item: requests.map((r) =>
      oursRequestToPmItem(
        r.name,
        r.method,
        r.url || "",
        parseKv(r.params),
        parseKv(r.headers),
        parseBody(r.body),
        r.preScript || "",
        r.testScript || ""
      )
    ),
  };
}

/** RequestItem → Postman item（导出时按树组织，保留文件夹层级） */
export function requestItemToPmItem(r: RequestItem): PmItem {
  return oursRequestToPmItem(
    r.name,
    r.method,
    r.url || "",
    parseKv(r.params),
    parseKv(r.headers),
    parseBody(r.body),
    r.preScript || "",
    r.testScript || ""
  );
}

/** 用预先组织好的 item 树（含嵌套文件夹）构建 Postman Collection */
export function buildPostmanCollectionTree(name: string, items: PmItem[]): PmCollection {
  return {
    info: {
      name,
      schema: "https://schema.postman.com/json/collection/v2.1.0/collection.json",
    },
    item: items,
  };
}

export function draftToPmItem(draft: RequestDraft): PmItem {
  return oursRequestToPmItem(
    draft.name,
    draft.method,
    draft.url,
    draft.params,
    draft.headers,
    draft.body,
    draft.preScript || "",
    draft.testScript || ""
  );
}

function parseKv(s: string | null): KeyValue[] {
  if (!s) return [];
  try {
    return JSON.parse(s) as KeyValue[];
  } catch {
    return [];
  }
}
function parseBody(s: string | null): RequestBody | null {
  if (!s) return null;
  try {
    return JSON.parse(s) as RequestBody;
  } catch {
    return null;
  }
}

export const PM_SCHEMA_URL =
  "https://schema.postman.com/json/collection/v2.1.0/collection.json";

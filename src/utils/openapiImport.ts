import { parse as parseYaml } from "yaml";
import type { AuthConfig, KeyValue, RequestBody } from "@/types";

/**
 * OpenAPI 3.x / Swagger 2.0 → 集合树 解析器。
 * 输出与 collectionStore.importTree 兼容的节点结构：
 *   { type: "folder" | "request", name, children?, request? }
 * 请求 URL 使用 servers[0] + path 拼成绝对地址（导入后可直接发送）。
 */

export interface ImportedNode {
  type: "folder" | "request";
  name: string;
  children?: ImportedNode[];
  request?: {
    name: string;
    method: string;
    url: string;
    params: KeyValue[];
    headers: KeyValue[];
    body: RequestBody | null;
    preScript?: string;
    testScript?: string;
    auth?: AuthConfig | null;
  };
}

const METHODS = ["get", "post", "put", "delete", "patch", "head", "options"] as const;

/** 解析 OpenAPI/Swagger 文本（JSON 或 YAML），返回可导入的集合树 */
export function parseOpenApiText(text: string): { title: string; tree: ImportedNode; count: number } {
  const doc = parseDoc(text);
  const ver = String(doc.openapi ?? doc.swagger ?? "");
  if (!ver) {
    throw new Error("未识别到 openapi/swagger 版本字段，请确认这是 OpenAPI/Swagger 文档");
  }

  const title = (doc.info?.title as string) || "OpenAPI Import";
  const baseUrl = resolveBaseUrl(doc);

  // tag → 子节点列表（保持首次出现顺序）
  const byTag = new Map<string, ImportedNode[]>();
  const noTag: ImportedNode[] = [];
  let count = 0;

  const paths = (doc.paths ?? {}) as Record<string, any>;
  for (const [path, pathItem] of Object.entries(paths)) {
    if (!pathItem || typeof pathItem !== "object") continue;

    // path 级公共参数（合并到每个操作，操作级优先）
    const pathLevelParams = Array.isArray(pathItem.parameters) ? pathItem.parameters : [];

    for (const m of METHODS) {
      const op = pathItem[m];
      if (!op || typeof op !== "object") continue;

      const method = m.toUpperCase();
      const name = String(op.summary || op.operationId || `${method} ${path}`);
      const merged = mergeParams(pathLevelParams, Array.isArray(op.parameters) ? op.parameters : []);
      const params: KeyValue[] = [];
      const headers: KeyValue[] = [];
      for (const p of merged) {
        const kv = {
          key: String(p.name ?? ""),
          value: String(p.example ?? p.schema?.example ?? p.schema?.default ?? ""),
          enabled: true,
        };
        if (!kv.key) continue;
        if (p.in === "query") params.push(kv);
        else if (p.in === "header") headers.push(kv);
        // path/cookie 参数本工具暂不支持替换，跳过
      }

      const node: ImportedNode = {
        type: "request",
        name,
        request: {
          name,
          method,
          url: joinUrl(baseUrl, path),
          params,
          headers,
          body: extractBody(op, doc),
          preScript: "",
          testScript: "",
        },
      };
      count++;

      const tag = Array.isArray(op.tags) && op.tags.length ? String(op.tags[0]) : "";
      if (tag) {
        const list = byTag.get(tag) ?? [];
        list.push(node);
        byTag.set(tag, list);
      } else {
        noTag.push(node);
      }
    }
  }

  if (!count) {
    throw new Error("文档中没有可导入的 path 操作（paths 为空）");
  }

  const children: ImportedNode[] = [];
  for (const [tag, list] of byTag) children.push({ type: "folder", name: tag, children: list });
  children.push(...noTag);

  return { title, tree: { type: "folder", name: title, children }, count };
}

// ---- 内部工具 ----

function parseDoc(text: string): any {
  const trimmed = text.trim();
  if (trimmed.startsWith("{") || trimmed.startsWith("[")) {
    return JSON.parse(trimmed);
  }
  return parseYaml(trimmed);
}

/** servers（v3）或 host/basePath/schemes（v2）→ 基础 URL */
function resolveBaseUrl(doc: any): string {
  const servers = Array.isArray(doc.servers) ? doc.servers : [];
  const s = servers[0];
  if (s?.url) {
    // server 变量（{protocol}://{host}）用默认值替换
    let url = String(s.url);
    const vars = s.variables ?? {};
    for (const [k, v] of Object.entries(vars)) {
      const def = (v as any)?.default;
      if (def != null) url = url.replaceAll(`{${k}}`, String(def));
    }
    return url;
  }
  if (doc.host) {
    const scheme = Array.isArray(doc.schemes) && doc.schemes.length ? doc.schemes[0] : "https";
    const basePath = doc.basePath && doc.basePath !== "/" ? String(doc.basePath) : "";
    return `${scheme}://${doc.host}${basePath}`;
  }
  return "";
}

function joinUrl(base: string, path: string): string {
  if (!base) return path;
  if (path.startsWith("http://") || path.startsWith("https://")) return path;
  return base.replace(/\/+$/, "") + "/" + path.replace(/^\/+/, "");
}

/** path 级 + 操作级参数合并（操作级同名覆盖 path 级） */
function mergeParams(pathLevel: any[], opLevel: any[]): any[] {
  const out = [...pathLevel];
  for (const p of opLevel) {
    const idx = out.findIndex((x) => x.name === p.name && x.in === p.in);
    if (idx >= 0) out[idx] = p;
    else out.push(p);
  }
  return out;
}

/** requestBody（v3）/（v2 的 form 参数略）→ RequestBody */
function extractBody(op: any, doc: any): RequestBody | null {
  const content = op.requestBody?.content;
  if (!content || typeof content !== "object") return null;
  // 优先 json，否则第一个 key
  const mime =
    Object.keys(content).find((k) => k.includes("json")) ?? Object.keys(content)[0];
  if (!mime) return null;
  const media = content[mime] ?? {};
  let raw: string;
  if (media.example !== undefined) {
    raw = typeof media.example === "string" ? media.example : JSON.stringify(media.example, null, 2);
  } else if (media.examples && typeof media.examples === "object") {
    const first = Object.values(media.examples)[0] as any;
    const v = first?.value;
    raw = v === undefined ? "" : typeof v === "string" ? v : JSON.stringify(v, null, 2);
  } else {
    const example = schemaToExample(media.schema, doc, new Set());
    raw = example === undefined ? "" : JSON.stringify(example, null, 2);
  }
  return { mode: "raw", raw, mimeType: mime };
}

/**
 * JSON Schema → 示例值（example/default 优先，其次按类型生成最小值）。
 * 支持 $ref 一层解析（components/schemas），带环保护。
 */
export function schemaToExample(schema: any, doc: any, seen: Set<string>): any {
  if (!schema || typeof schema !== "object") return undefined;
  if (schema.example !== undefined) return schema.example;
  if (schema.default !== undefined) return schema.default;

  if (schema.$ref && typeof schema.$ref === "string") {
    if (seen.has(schema.$ref)) return null; // 循环引用
    seen.add(schema.$ref);
    const name = schema.$ref.split("/").pop();
    const target = doc?.components?.schemas?.[name] ?? doc?.definitions?.[name];
    return schemaToExample(target, doc, seen);
  }

  if (Array.isArray(schema.enum) && schema.enum.length) return schema.enum[0];

  switch (schema.type) {
    case "string":
      return "";
    case "number":
    case "integer":
      return 0;
    case "boolean":
      return false;
    case "array":
      return [schemaToExample(schema.items, doc, seen)];
    case "object": {
      const props = schema.properties;
      if (!props || typeof props !== "object") return {};
      const out: Record<string, any> = {};
      for (const [k, v] of Object.entries(props)) {
        out[k] = schemaToExample(v, doc, new Set(seen));
      }
      return out;
    }
    default:
      // 未标 type 但有 properties → 视为对象
      if (schema.properties) {
        const out: Record<string, any> = {};
        for (const [k, v] of Object.entries(schema.properties)) {
          out[k] = schemaToExample(v, doc, new Set(seen));
        }
        return out;
      }
      return null;
  }
}

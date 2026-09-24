import type { HttpRequestPayload } from "@/types";

/** 把已解析变量的请求 payload 转为 cURL 命令字符串 */
export function toCurl(req: HttpRequestPayload): string {
  const parts: string[] = ["curl"];
  // method
  if (req.method.toUpperCase() !== "GET") parts.push("-X", req.method.toUpperCase());

  // 构建完整 URL（带 query）
  let url = req.url;
  const activeParams = req.params.filter((p) => p.enabled && p.key);
  if (activeParams.length) {
    const sep = url.includes("?") ? "&" : "?";
    url += sep + activeParams.map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`).join("&");
  }
  parts.push(quote(url));

  // headers
  for (const h of req.headers.filter((x) => x.enabled && x.key)) {
    parts.push("-H", quote(`${h.key}: ${h.value}`));
  }

  // body
  if (req.body) {
    if (req.body.mode === "raw") {
      parts.push("-d", quote(req.body.raw));
    } else {
      const items = req.body.items.filter((x) => x.enabled && x.key);
      for (const it of items) {
        parts.push("-d", quote(`${encodeURIComponent(it.key)}=${encodeURIComponent(it.value)}`));
      }
    }
  }

  return parts.join(" \\\n  ");
}

/** 复制文本到剪贴板（优先 Tauri，退化 navigator.clipboard） */
export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

/** shell 风格转义：含特殊字符则用单引号包裹，内部单引号转义 */
function quote(s: string): string {
  if (/^[A-Za-z0-9_\-:.\/=?&@%~+,]+$/.test(s)) return s;
  return "'" + s.replace(/'/g, "'\\''") + "'";
}

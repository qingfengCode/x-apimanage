import type { AuthConfig, BasicAuth, BearerAuth, KeyValue, RequestBody } from "@/types";

/**
 * cURL 命令 → 请求草稿字段 解析器。
 * 目标是覆盖开发者日常从浏览器「Copy as cURL」得到的命令：
 *   - bash 单/双引号、$'...' ANSI-C 字符串、\<换行> 续行
 *   - Windows cmd 的 ^<换行> 续行与 PowerShell 反引号续行
 *   - -X/-H/-d/--data-raw/-F/-u/-G/-b/-A/-e/-I 等常用参数
 * 不追求完整复刻 curl 语义（如 --data @file 这类引用本地文件的仅按原文导入）。
 */

export interface ParsedCurl {
  method: string;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  body: RequestBody | null;
  auth: AuthConfig | null;
  /** 推断的请求名（如 "GET /api/users"） */
  name: string;
}

/** 词法分析：把整条命令拆为参数数组（引号去除、续行合并） */
export function tokenizeCurl(cmd: string): string[] {
  let s = cmd.replace(/\\\r?\n/g, " "); // bash 续行
  s = s.replace(/\^\r?\n/g, " "); // cmd.exe 续行
  s = s.replace(/`\r?\n/g, " "); // PowerShell 续行

  const tokens: string[] = [];
  let cur = "";
  let hasToken = false;
  const push = () => {
    if (hasToken) {
      tokens.push(cur);
      cur = "";
      hasToken = false;
    }
  };

  let i = 0;
  const n = s.length;
  while (i < n) {
    const c = s[i];

    if (c === "'" || (c === "$" && s[i + 1] === "'")) {
      // 单引号 / $'...'：原文直到下一个单引号（'\'' 转义在内部处理）
      if (c === "$") i++;
      i++;
      hasToken = true;
      while (i < n) {
        if (s[i] === "'") {
          if (s[i + 1] === "\\" && s[i + 2] === "'" && s[i + 3] === "'") {
            // '\'' → 字面单引号
            cur += "'";
            i += 4;
          } else break;
        } else {
          cur += s[i];
          i++;
        }
      }
      i++; // 跳过收尾引号
      continue;
    }

    if (c === '"') {
      // 双引号：支持 \" 与 \\ 转义
      i++;
      hasToken = true;
      while (i < n && s[i] !== '"') {
        if (s[i] === "\\" && (s[i + 1] === '"' || s[i + 1] === "\\")) {
          cur += s[i + 1];
          i += 2;
        } else {
          cur += s[i];
          i++;
        }
      }
      i++; // 跳过收尾引号
      continue;
    }

    if (/\s/.test(c)) {
      push();
      i++;
      continue;
    }

    cur += c;
    hasToken = true;
    i++;
  }
  push();
  return tokens;
}

/** 带值参数表：短/长参数 → 是否需要值 */
const FLAGS_WITH_VALUE = new Set([
  "-X", "--request",
  "-H", "--header",
  "-d", "--data", "--data-raw", "--data-ascii", "--data-binary", "--data-urlencode",
  "-F", "--form",
  "-u", "--user",
  "-b", "--cookie",
  "-A", "--user-agent",
  "-e", "--referer",
  "--url",
  "--content-type",
  "-t", "--connect-timeout",
  "-m", "--max-time",
  "-x", "--proxy",
  "-T", "--upload-file",
  "-o", "--output",
  "-w", "--write-out",
  "--retry",
  "-c", "--cookie-jar",
  "-E", "--cert",
  "--key",
]);

/** 无值开关（解析时跳过） */
const FLAG_BOOL = new Set([
  "-G", "--get", "-I", "--head", "-L", "--location", "-k", "--insecure",
  "-s", "--silent", "-S", "--show-error", "-v", "--verbose", "-#",
  "-i", "--include", "--compressed", "-f", "--fail", "-4", "-6",
  "-N", "--no-buffer", "--http1.1", "--http2", "-g", "--globoff",
]);

export function parseCurlCommand(cmd: string): ParsedCurl {
  const tokens = tokenizeCurl(cmd.trim());

  let method = "";
  let url = "";
  const rawHeaders: { key: string; value: string }[] = [];
  const dataParts: string[] = [];
  const formParts: string[] = [];
  let basicUser = "";
  let forceGet = false;
  let headOnly = false;

  // 跳过开头的 curl 可执行名（可能带路径，如 /usr/bin/curl、curl.exe）
  let start = 0;
  if (tokens.length && /(^|[\\/])curl(\.exe)?$/.test(tokens[0])) start = 1;

  for (let i = start; i < tokens.length; i++) {
    const t = tokens[i];
    const next = () => {
      i++;
      return tokens[i] ?? "";
    };

    if (FLAGS_WITH_VALUE.has(t)) {
      const v = next();
      switch (t) {
        case "-X": case "--request":
          method = v.toUpperCase();
          break;
        case "-H": case "--header": {
          const idx = v.indexOf(":");
          if (idx > 0) {
            const key = v.slice(0, idx).trim();
            const val = v.slice(idx + 1).trim();
            // 重复头（curl 会发送多次）→ 合并显示为一行
            rawHeaders.push({ key, value: val });
          }
          break;
        }
        case "--data-urlencode":
          dataParts.push(v); // 语义上应 urlencode，此处保留原文便于编辑
          break;
        case "-d": case "--data": case "--data-raw": case "--data-ascii": case "--data-binary":
          dataParts.push(v);
          break;
        case "-F": case "--form":
          formParts.push(v);
          break;
        case "-u": case "--user":
          basicUser = v;
          break;
        case "-b": case "--cookie": {
          if (v.includes("=")) rawHeaders.push({ key: "Cookie", value: v });
          break;
        }
        case "-A": case "--user-agent":
          rawHeaders.push({ key: "User-Agent", value: v });
          break;
        case "-e": case "--referer":
          rawHeaders.push({ key: "Referer", value: v });
          break;
        case "--url":
          url = v;
          break;
        case "--content-type":
          rawHeaders.push({ key: "Content-Type", value: v });
          break;
        // 超时/代理等与本工具无关的参数：吞掉值即可
        default:
          break;
      }
      continue;
    }

    if (FLAG_BOOL.has(t)) {
      if (t === "-G" || t === "--get") forceGet = true;
      if (t === "-I" || t === "--head") headOnly = true;
      continue;
    }

    // 组合短参数，如 -sSk
    if (t.startsWith("-") && t.length > 2 && !t.startsWith("--")) {
      const chars = t.slice(1).split("");
      const allKnown = chars.every((c) => FLAG_BOOL.has("-" + c));
      if (allKnown) {
        if (chars.includes("G")) forceGet = true;
        if (chars.includes("I")) headOnly = true;
        continue;
      }
    }

    // 位置参数：第一个非参数 token 视为 URL
    if (!url && /^(https?:\/\/|[\w.-]+(:\d+)?(\/|$))/i.test(t)) {
      url = t;
    }
  }

  // URL 规范化
  if (url && !/^[a-z][a-z0-9+.-]*:\/\//i.test(url)) url = "https://" + url;

  // query 拆分为 params（与 ParamsTab 双向同步的约定一致：URL 自带 query）
  const params: KeyValue[] = [];
  let cleanUrl = url;
  const qIdx = url.indexOf("?");
  if (qIdx >= 0) {
    cleanUrl = url.slice(0, qIdx);
    const query = url.slice(qIdx + 1);
    for (const pair of query.split("&")) {
      if (!pair) continue;
      const eq = pair.indexOf("=");
      const k = eq >= 0 ? pair.slice(0, eq) : pair;
      const v = eq >= 0 ? pair.slice(eq + 1) : "";
      params.push({ key: safeDecode(k), value: safeDecode(v), enabled: true });
    }
  }

  // 头部处理：识别 Authorization → 转为 Auth 配置
  let auth: AuthConfig | null = null;
  const headers: KeyValue[] = [];
  for (const h of rawHeaders) {
    if (h.key.toLowerCase() === "authorization") {
      auth = authFromHeader(h.value) ?? auth;
      if (!auth) headers.push({ key: h.key, value: h.value, enabled: true });
      continue;
    }
    // 同名头去重（后出现的覆盖）
    const existing = headers.find((x) => x.key.toLowerCase() === h.key.toLowerCase());
    if (existing) existing.value = h.value;
    else headers.push({ key: h.key, value: h.value, enabled: true });
  }

  // Basic 认证（-u）
  if (basicUser) {
    const idx = basicUser.indexOf(":");
    auth = {
      type: "basic",
      username: idx >= 0 ? basicUser.slice(0, idx) : basicUser,
      password: idx >= 0 ? basicUser.slice(idx + 1) : "",
    } satisfies BasicAuth;
  }

  // body
  let body: RequestBody | null = null;
  if (formParts.length) {
    body = {
      mode: "multipart",
      items: formParts.map((p) => {
        const idx = p.indexOf("=");
        return {
          key: idx >= 0 ? p.slice(0, idx) : p,
          value: idx >= 0 ? p.slice(idx + 1) : "",
          enabled: true,
        };
      }),
    };
  } else if (dataParts.length) {
    const raw = dataParts.join("&");
    const ct = headers.find((h) => h.key.toLowerCase() === "content-type")?.value ?? "";
    const looksJson = ct.includes("json") || /^\s*[{\[]/.test(raw);
    // 无 Content-Type 时 curl -d 默认按 x-www-form-urlencoded 发送：
    // 整体是 k=v&k2=v2 形式则识别为表单
    const kvPairs = raw.split("&");
    const looksForm =
      !ct && kvPairs.length >= 1 && kvPairs.every((d) => /^[^=&]+=[^&]*$/.test(d)) && !/^\s*[{\[]/.test(raw);
    if (looksJson || ct) {
      body = { mode: "raw", raw, mimeType: ct || "application/json" };
    } else if (looksForm) {
      body = {
        mode: "form",
        items: kvPairs.map((d) => {
          const idx = d.indexOf("=");
          return {
            key: idx >= 0 ? d.slice(0, idx) : d,
            value: idx >= 0 ? d.slice(idx + 1) : "",
            enabled: true,
          };
        }),
      };
    } else {
      body = { mode: "raw", raw, mimeType: ct || "text/plain" };
    }
  }

  // method 推断
  let finalMethod = method;
  if (!finalMethod) {
    if (headOnly) finalMethod = "HEAD";
    else if (forceGet) finalMethod = "GET";
    else if (body) finalMethod = "POST";
    else finalMethod = "GET";
  }

  // 名称推断：METHOD + path
  let pathName = "/";
  try {
    pathName = new URL(cleanUrl).pathname || "/";
  } catch {
    const m = cleanUrl.match(/\/[^\s?]*/);
    if (m) pathName = m[0];
  }
  const name = `${finalMethod} ${pathName}`.slice(0, 64);

  return { method: finalMethod, url: cleanUrl, params, headers, body, auth, name };
}

/** Authorization 头值 → AuthConfig（识别 Bearer/Basic），识别不了返回 null（保留原头） */
function authFromHeader(value: string): AuthConfig | null {
  const m = value.match(/^(\w+)\s+(.+)$/);
  if (m) {
    const scheme = m[1].toLowerCase();
    if (scheme === "bearer") {
      return { type: "bearer", token: m[2], prefix: "Bearer" } satisfies BearerAuth;
    }
    if (scheme === "basic") {
      try {
        const decoded = atob(m[2]);
        const idx = decoded.indexOf(":");
        return {
          type: "basic",
          username: idx >= 0 ? decoded.slice(0, idx) : decoded,
          password: idx >= 0 ? decoded.slice(idx + 1) : "",
        } satisfies BasicAuth;
      } catch {
        return null;
      }
    }
  }
  return null;
}

function safeDecode(s: string): string {
  try {
    return decodeURIComponent(s.replace(/\+/g, " "));
  } catch {
    return s;
  }
}

/** 校验命令是否大致是可解析的 cURL（用于导入弹窗的实时反馈） */
export function isLikelyCurl(cmd: string): boolean {
  return /(^|\s|")curl(\.exe)?"?(\s|$)/i.test(cmd.trim());
}

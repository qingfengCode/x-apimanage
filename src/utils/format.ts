/** 格式化字节数 */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

/** 格式化耗时（毫秒） */
export function formatTime(ms: number): string {
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

/** 格式化时间戳为本地时间 */
export function formatDateTime(ts: number): string {
  const d = new Date(ts);
  const pad = (n: number) => n.toString().padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(
    d.getHours()
  )}:${pad(d.getMinutes())}:${pad(d.getSeconds())}`;
}

/** 根据 HTTP 状态码返回颜色类 */
export function statusColor(status: number): string {
  if (status >= 200 && status < 300) return "text-accent-green";
  if (status >= 300 && status < 400) return "text-accent-blue";
  if (status >= 400 && status < 500) return "text-accent-orange";
  if (status >= 500) return "text-accent-red";
  return "text-app-muted";
}

/** 根据 HTTP 状态码返回胶囊底色类（浅底 + 同色文字） */
export function statusPillColor(status: number): string {
  if (status >= 200 && status < 300) return "bg-accent-green/15 text-accent-green";
  if (status >= 300 && status < 400) return "bg-accent-blue/15 text-accent-blue";
  if (status >= 400 && status < 500) return "bg-accent-orange/15 text-accent-orange";
  if (status >= 500) return "bg-accent-red/15 text-accent-red";
  return "bg-app-hover text-app-muted";
}

/** 根据 HTTP 方法返回颜色类 */
export function methodColor(method: string): string {
  switch (method.toUpperCase()) {
    case "GET":
      return "text-accent-green";
    case "POST":
      return "text-accent-orange";
    case "PUT":
      return "text-accent-blue";
    case "DELETE":
      return "text-accent-red";
    case "PATCH":
      return "text-accent-purple";
    default:
      return "text-app-muted";
  }
}

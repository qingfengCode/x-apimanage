import type { ResponseBody } from "@/types";

/** 二进制响应体（base64 承载） */
export type BinaryResponseBody = Extract<ResponseBody, { kind: "binary" }>;

/** 美化 JSON 文本 */
export function prettyJson(text: string): string {
  try {
    return JSON.stringify(JSON.parse(text), null, 2);
  } catch {
    return text;
  }
}

/** 根据 mime 推断 Monaco 语言 */
export function mimeToLanguage(mime: string): string {
  const m = mime.toLowerCase();
  if (m.includes("json")) return "json";
  if (m.includes("html")) return "html";
  if (m.includes("xml")) return "xml";
  if (m.includes("javascript") || m.includes("ecmascript")) return "javascript";
  if (m.includes("css")) return "css";
  if (m.includes("markdown")) return "markdown";
  if (m.includes("yaml")) return "yaml";
  return "text";
}

/** 响应体统一取文本 */
export function responseText(body: ResponseBody): string {
  if (body.kind === "text") return body.text;
  return `[binary data, ${body.mime || "unknown"}, ${Math.ceil((body.base64.length * 3) / 4)} bytes]`;
}

/** 根据响应 mime 尝试美化文本 */
export function prettifyResponseText(body: ResponseBody): string {
  const raw = responseText(body);
  if (body.kind !== "text") return raw;
  const mime = body.mime.toLowerCase();
  if (mime.includes("json")) return prettyJson(raw);
  return raw;
}

// ---- 媒体类型判定 ----

/** 去掉 mime 参数，仅保留 type/subtype（小写） */
export function baseMime(mime: string): string {
  return (mime || "").split(";")[0].trim().toLowerCase();
}

export function isImageMime(mime: string): boolean {
  return baseMime(mime).startsWith("image/");
}

export function isAudioMime(mime: string): boolean {
  return baseMime(mime).startsWith("audio/");
}

export function isVideoMime(mime: string): boolean {
  return baseMime(mime).startsWith("video/");
}

/** content-type 转 base64 data url（用于图片预览） */
export function toDataUrl(body: BinaryResponseBody): string {
  return `data:${body.mime || "application/octet-stream"};base64,${body.base64}`;
}

// ---- base64 / 字节流转换 ----

/** base64 解码为字节数组（容忍换行与空白） */
export function base64ToBytes(base64: string): Uint8Array {
  const bin = atob(base64.replace(/\s/g, ""));
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** 字节数组编码为 base64（分块拼接，避免超长参数爆栈） */
export function bytesToBase64(bytes: Uint8Array): string {
  const CHUNK = 0x8000;
  let bin = "";
  for (let i = 0; i < bytes.length; i += CHUNK) {
    bin += String.fromCharCode(...bytes.subarray(i, i + CHUNK));
  }
  return btoa(bin);
}

/** 字节数组转 Blob URL（调用方负责在不再使用时 revokeObjectURL） */
export function bytesToBlobUrl(bytes: Uint8Array, mime: string): string {
  const buf =
    bytes.byteOffset === 0 && bytes.byteLength === bytes.buffer.byteLength
      ? bytes.buffer
      : bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
  const blob = new Blob([buf as ArrayBuffer], { type: mime || "application/octet-stream" });
  return URL.createObjectURL(blob);
}

// ---- 音频：容器格式 vs 裸 PCM ----

/** 可直接交给 <audio> 解码的容器格式关键字 → 扩展名（顺序敏感，更具体者在前） */
const AUDIO_CONTAINER_EXT: [string, string][] = [
  ["x-wav", ".wav"],
  ["wav", ".wav"],
  ["wave", ".wav"],
  ["mpeg", ".mp3"],
  ["mp3", ".mp3"],
  ["x-flac", ".flac"],
  ["flac", ".flac"],
  ["ogg", ".ogg"],
  ["opus", ".opus"],
  ["x-m4a", ".m4a"],
  ["m4a", ".m4a"],
  ["mp4", ".m4a"],
  ["aac", ".aac"],
  ["webm", ".webm"],
  ["3gpp", ".3gp"],
  ["amr", ".amr"],
  ["aiff", ".aiff"],
];

/** 裸采样流（无容器头）的 mime 关键字 */
const RAW_PCM_HINTS = ["pcm", "l16", "l8", "s16le", "s16be", "x-raw", "/raw", "basic"];

export type AudioKind = "container" | "pcm" | "unknown";

/**
 * 判定音频响应形态：
 * - container：wav/mp3/ogg… 自带容器头，播放器可直接解码
 * - pcm：裸采样数据（如智谱 TTS 默认的 response_format=pcm），需补 WAV 头才能播放
 * - unknown：mime 不足以判断（如 application/octet-stream），交给魔数嗅探
 */
export function audioKindOf(mime: string): AudioKind {
  const m = baseMime(mime);
  if (!m) return "unknown";
  // video/* 交给视频分支，避免 mp4 这类共用关键字被当成音频
  if (m.startsWith("video/")) return "unknown";
  if (m.startsWith("audio/")) {
    if (RAW_PCM_HINTS.some((h) => m.includes(h))) return "pcm";
    if (AUDIO_CONTAINER_EXT.some(([k]) => m.includes(k))) return "container";
    // 声明为 audio/* 但不认识具体格式时，多数服务端发的是裸采样流
    return "pcm";
  }
  // 少数不带 audio/ 前缀但确实有音频容器头的类型
  if (m === "application/ogg" || m === "application/x-ogg") return "container";
  return "unknown";
}

/** 按魔数嗅探音频容器类型，返回可直接播放的 mime；无法识别返回 null */
export function sniffAudioContainer(bytes: Uint8Array): string | null {
  const ascii = (off: number, len: number) =>
    String.fromCharCode(...bytes.subarray(off, off + len));

  if (bytes.length >= 12) {
    if (ascii(0, 4) === "RIFF" && ascii(8, 4) === "WAVE") return "audio/wav";
    if (ascii(0, 4) === "OggS") return "audio/ogg";
    if (ascii(0, 4) === "fLaC") return "audio/flac";
    if (ascii(4, 4) === "ftyp") return "audio/mp4";
  }
  if (bytes.length < 4) return null;
  if (ascii(0, 3) === "ID3") return "audio/mpeg";
  if (ascii(0, 4) === "#!AM") return "audio/amr";
  // MPEG 帧同步：11 个连续 1 比特
  if (bytes[0] === 0xff && (bytes[1] & 0xe0) === 0xe0) return "audio/mpeg";
  // WebM / Matroska 的 EBML 头
  if (bytes[0] === 0x1a && bytes[1] === 0x45 && bytes[2] === 0xdf && bytes[3] === 0xa3) {
    return "audio/webm";
  }
  return null;
}

/** 仅解码 base64 前缀用于魔数嗅探，避免为大响应做一次完整解码 */
export function sniffAudioContainerFromBase64(base64: string): string | null {
  const clean = base64.replace(/\s/g, "");
  const head = clean.slice(0, 48);
  const slice = head.slice(0, head.length - (head.length % 4));
  try {
    return sniffAudioContainer(base64ToBytes(slice));
  } catch {
    return null;
  }
}

/** 裸 PCM 的播放参数 */
export interface PcmFormat {
  sampleRate: number;
  channels: number;
  bitsPerSample: number;
}

/** 默认参数：智谱 GLM-TTS 建议 24000Hz 采样，返回单声道 16bit 裸 PCM */
export const DEFAULT_PCM_FORMAT: PcmFormat = {
  sampleRate: 24000,
  channels: 1,
  bitsPerSample: 16,
};

/** 为裸 PCM 数据补 44 字节 WAV(RIFF) 头，使其可被浏览器播放器解码 */
export function pcmToWav(pcm: Uint8Array, fmt: PcmFormat): Uint8Array {
  const { sampleRate, channels, bitsPerSample } = fmt;
  const blockAlign = (channels * bitsPerSample) / 8;
  const byteRate = sampleRate * blockAlign;

  const out = new Uint8Array(44 + pcm.length);
  const view = new DataView(out.buffer);
  const ascii = (offset: number, s: string) => {
    for (let i = 0; i < s.length; i++) out[offset + i] = s.charCodeAt(i);
  };

  ascii(0, "RIFF");
  view.setUint32(4, 36 + pcm.length, true);
  ascii(8, "WAVE");
  ascii(12, "fmt ");
  view.setUint32(16, 16, true); // fmt 块长度
  view.setUint16(20, 1, true); // 编码格式：1 = PCM
  view.setUint16(22, channels, true);
  view.setUint32(24, sampleRate, true);
  view.setUint32(28, byteRate, true);
  view.setUint16(32, blockAlign, true);
  view.setUint16(34, bitsPerSample, true);
  ascii(36, "data");
  view.setUint32(40, pcm.length, true);
  out.set(pcm, 44);

  return out;
}

/** 裸 PCM 字节数 → 时长（秒） */
export function pcmDuration(bytes: number, fmt: PcmFormat): number {
  const bytesPerSecond = fmt.sampleRate * fmt.channels * (fmt.bitsPerSample / 8);
  return bytesPerSecond > 0 ? bytes / bytesPerSecond : 0;
}

// ---- 保存到文件：扩展名与建议文件名 ----

/** 文本类响应扩展名 */
const TEXT_EXT_BY_MIME: [string, string][] = [
  ["json", ".json"],
  ["html", ".html"],
  ["xml", ".xml"],
  ["javascript", ".js"],
  ["css", ".css"],
  ["csv", ".csv"],
  ["markdown", ".md"],
  ["yaml", ".yaml"],
  ["plain", ".txt"],
  ["png", ".png"],
  ["jpeg", ".jpg"],
  ["gif", ".gif"],
  ["svg", ".svg"],
  ["webp", ".webp"],
  ["pdf", ".pdf"],
  ["zip", ".zip"],
];

/** 按 mime 推断保存扩展名（按 audio/video/文本三级判定，避免互相抢占） */
export function extFromMime(mime: string): string {
  const m = baseMime(mime);
  if (!m) return ".bin";
  if (m.startsWith("audio/")) {
    if (RAW_PCM_HINTS.some((h) => m.includes(h))) return ".pcm";
    for (const [k, ext] of AUDIO_CONTAINER_EXT) if (m.includes(k)) return ext;
    return ".audio";
  }
  if (m.startsWith("video/")) {
    if (m.includes("quicktime")) return ".mov";
    if (m.includes("x-msvideo")) return ".avi";
    if (m.includes("webm")) return ".webm";
    if (m.includes("mp4")) return ".mp4";
    return ".video";
  }
  for (const [k, ext] of TEXT_EXT_BY_MIME) if (m.includes(k)) return ext;
  return ".bin";
}

/** 只取文件名部分，防止服务端返回路径造成目录穿越 */
function basename(p: string): string | null {
  const name = p.split(/[/\\]/).filter(Boolean).pop() || "";
  if (!name || name === "." || name === "..") return null;
  return name;
}

/**
 * 解析 Content-Disposition 中的文件名。
 * 支持 filename*=UTF-8''%E4%B8%AD.wav（RFC 5987）与 filename="a.wav"。
 */
export function filenameFromContentDisposition(header: string | undefined): string | null {
  if (!header) return null;
  const star = /filename\*\s*=\s*([^;]+)/i.exec(header);
  if (star) {
    const raw = star[1].trim().replace(/^["']|["']$/g, "");
    const encoded = raw.includes("''") ? raw.slice(raw.indexOf("''") + 2) : raw;
    try {
      return basename(decodeURIComponent(encoded));
    } catch {
      return basename(encoded);
    }
  }
  const plain = /filename\s*=\s*"?([^";]+)"?/i.exec(header);
  if (plain) return basename(plain[1].trim());
  return null;
}

/**
 * 推断保存响应体时的建议文件名：Content-Disposition → URL 末段 → response.<ext>
 * @param forceExt 强制扩展名（如裸 PCM 导出为 WAV 时传 ".wav"）
 */
export function suggestedFileName(
  headers: [string, string][],
  url: string,
  mime: string,
  forceExt?: string
): string {
  const cd = headers.find(([k]) => k.toLowerCase() === "content-disposition");
  let base = filenameFromContentDisposition(cd?.[1]);

  if (!base) {
    try {
      const last = new URL(url).pathname.split("/").filter(Boolean).pop() || "";
      if (last && /\.[A-Za-z0-9]{1,6}$/.test(last)) base = last;
    } catch {
      /* url 非法时忽略，退回默认名 */
    }
  }

  const ext = forceExt ?? extFromMime(mime);
  if (!base) return `response${ext}`;
  if (forceExt && !base.toLowerCase().endsWith(forceExt.toLowerCase())) {
    // 换扩展名（如 speech.pcm → speech.wav），保留原主名
    return base.replace(/\.[A-Za-z0-9]{1,6}$/, "") + forceExt;
  }
  return base;
}

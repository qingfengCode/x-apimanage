<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { AlertTriangle, Image as ImageIcon, Check, Copy, FileText, Clock, Music, Save, Search, Video, Weight } from "lucide-vue-next";
import type { HttpResponsePayload } from "@/types";
import { formatBytes, formatTime, statusColor, statusPillColor } from "@/utils/format";
import {
  DEFAULT_PCM_FORMAT,
  audioKindOf,
  base64ToBytes,
  baseMime,
  bytesToBase64,
  bytesToBlobUrl,
  isImageMime,
  isVideoMime,
  mimeToLanguage,
  pcmDuration,
  pcmToWav,
  prettifyResponseText,
  responseText,
  sniffAudioContainerFromBase64,
  suggestedFileName,
  toDataUrl,
  type PcmFormat,
} from "@/utils/mime";
import { copyText } from "@/utils/curl";
import { useToast } from "@/composables/useToast";
import { useHistoryStore } from "@/stores/history";
import MonacoEditor from "@/components/editor/MonacoEditor.vue";

const historyStore = useHistoryStore();

const props = defineProps<{ response: HttpResponsePayload }>();

type View = "body" | "headers";

const view = ref<View>("body");
const pretty = ref(true);

/** 超过该字符数时截断显示，避免 Monaco 渲染大文本卡顿 */
const MAX_DISPLAY_CHARS = 300_000;
const showFull = ref(false);

const language = computed(() =>
  props.response.body.kind === "text" ? mimeToLanguage(props.response.body.mime) : "text"
);

const rawText = computed(() => {
  return pretty.value ? prettifyResponseText(props.response.body) : responseText(props.response.body);
});

/** 是否因为过大被截断 */
const truncated = computed(() => rawText.value.length > MAX_DISPLAY_CHARS);

const displayText = computed(() => {
  if (truncated.value && !showFull.value) return rawText.value.slice(0, MAX_DISPLAY_CHARS);
  return rawText.value;
});

const isImage = computed(() => {
  const b = props.response.body;
  return b.kind === "binary" && isImageMime(b.mime);
});

const imgDataUrl = computed(() => {
  const b = props.response.body;
  if (b.kind !== "binary") return "";
  return toDataUrl(b);
});

// ---- 「文件型」响应：音频 / 视频播放 ----

/** 二进制响应体（非二进制为 null） */
const binaryBody = computed(() =>
  props.response.body.kind === "binary" ? props.response.body : null
);

const videoMime = computed(() => {
  const b = binaryBody.value;
  return b && isVideoMime(b.mime) ? baseMime(b.mime) : "";
});

/** 手动指定的播放形态，用于 content-type 不可信时兜底（随响应切换重置为自动判定） */
const audioOverride = ref<"container" | "pcm" | null>(null);

/** 裸 PCM 播放参数：智谱 GLM-TTS 建议 24000Hz，默认返回单声道 16bit 裸 PCM */
const pcmFormat = ref<PcmFormat>({ ...DEFAULT_PCM_FORMAT });

const SAMPLE_RATES = [8000, 16000, 22050, 24000, 32000, 44100, 48000];

/**
 * 音频播放方案：
 * - container：自带容器头（wav/mp3/ogg…），播放器可直接解码
 * - pcm：裸采样流，补 44 字节 WAV 头后播放
 * - null：不是、或无法判定为音频
 */
const audioPlan = computed<{ mode: "container"; mime: string } | { mode: "pcm" } | null>(() => {
  const b = binaryBody.value;
  // 图片/视频有自己的展示分支，不参与音频判定
  if (!b || videoMime.value || isImage.value) return null;

  if (audioOverride.value === "pcm") return { mode: "pcm" };
  if (audioOverride.value === "container") {
    return {
      mode: "container",
      mime: sniffAudioContainerFromBase64(b.base64) || baseMime(b.mime) || "audio/wav",
    };
  }

  const declared = audioKindOf(b.mime);
  if (declared === "container") return { mode: "container", mime: baseMime(b.mime) };
  // mime 不可信时按魔数嗅探：部分服务端对 wav/mp3 也返回 application/octet-stream
  const sniffed = sniffAudioContainerFromBase64(b.base64);
  if (sniffed) return { mode: "container", mime: sniffed };
  return declared === "pcm" ? { mode: "pcm" } : null;
});

const audioUrl = ref("");
const audioFailed = ref(false);
const audioError = ref("");
const audioDuration = ref(0);

function revokeAudioUrl() {
  if (audioUrl.value) {
    URL.revokeObjectURL(audioUrl.value);
    audioUrl.value = "";
  }
}

/** 按当前方案生成可播放的 Blob URL（裸 PCM 先补 WAV 头） */
function buildAudioUrl() {
  revokeAudioUrl();
  audioFailed.value = false;
  audioError.value = "";
  audioDuration.value = 0;

  const b = binaryBody.value;
  const plan = audioPlan.value;
  if (!b || !plan) return;
  try {
    const bytes = base64ToBytes(b.base64);
    audioUrl.value =
      plan.mode === "pcm"
        ? bytesToBlobUrl(pcmToWav(bytes, pcmFormat.value), "audio/wav")
        : bytesToBlobUrl(bytes, plan.mime);
  } catch (e) {
    audioError.value = String(e);
  }
}

/** 裸 PCM 时长（按字节数与播放参数直接算出，无需等播放器解码） */
const pcmSeconds = computed(() =>
  audioPlan.value?.mode === "pcm" ? pcmDuration(props.response.size, pcmFormat.value) : 0
);

function onAudioMeta(e: Event) {
  const el = e.target as HTMLAudioElement;
  if (Number.isFinite(el.duration) && el.duration > 0) audioDuration.value = el.duration;
}

// 视频同样走 Blob URL，避免把整个 base64 塞进 DOM 属性
const videoUrl = ref("");
function buildVideoUrl() {
  if (videoUrl.value) {
    URL.revokeObjectURL(videoUrl.value);
    videoUrl.value = "";
  }
  const b = binaryBody.value;
  if (!b || !videoMime.value) return;
  try {
    videoUrl.value = bytesToBlobUrl(base64ToBytes(b.base64), videoMime.value);
  } catch {
    /* 解码失败时保持占位，不阻塞其它视图 */
  }
}

// 新响应到达时清空手动兜底。用 sync 保证先于音频 URL 重建生效，
// 否则大体积音频会按上一次的解析方式白白解码一遍。
watch(() => props.response, () => (audioOverride.value = null), { flush: "sync" });

watch(audioPlan, buildAudioUrl, { immediate: true });
watch(pcmFormat, buildAudioUrl, { deep: true });
watch(videoMime, buildVideoUrl, { immediate: true });
onBeforeUnmount(() => {
  revokeAudioUrl();
  if (videoUrl.value) URL.revokeObjectURL(videoUrl.value);
});

// 切换响应时默认 Body 视图
watch(
  () => props.response,
  () => {
    view.value = "body";
    showFull.value = false;
  }
);

const toast = useToast();

/** 复制反馈：按钮短暂变 Check */
const copiedBody = ref(false);
async function copyBody() {
  // 被截断且未展开时，复制完整内容而非截断片段，避免用户误以为复制了全文
  const fullCopy = truncated.value && !showFull.value;
  const text = fullCopy ? rawText.value : displayText.value;
  if (await copyText(text)) {
    copiedBody.value = true;
    if (fullCopy) toast.push("已复制完整响应体（未截断）", "success", 2000);
    setTimeout(() => (copiedBody.value = false), 1500);
  }
}

/** 复制单个响应头 */
const copiedHeader = ref<string | null>(null);
async function copyHeader(name: string, value: string) {
  if (await copyText(`${name}: ${value}`)) {
    copiedHeader.value = name;
    toast.push(`已复制响应头 ${name}`, "success", 2000);
    setTimeout(() => {
      if (copiedHeader.value === name) copiedHeader.value = null;
    }, 1500);
  }
}

// ---- 响应体保存到文件（文本原样，二进制 base64 交给 Rust 落盘） ----
const savedBody = ref(false);

async function saveBodyToFile() {
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const b = props.response.body;
    const { headers, url } = props.response;

    // 文本按显示内容保存；裸 PCM 先补 WAV 头再落盘（否则存下来的裸流无法直接播放）
    let text: string | null = null;
    let base64 = "";
    let defaultPath: string;

    if (b.kind === "text") {
      text = rawText.value;
      defaultPath = suggestedFileName(headers, url, b.mime || "text/plain");
    } else if (audioPlan.value?.mode === "pcm") {
      base64 = bytesToBase64(pcmToWav(base64ToBytes(b.base64), pcmFormat.value));
      defaultPath = suggestedFileName(headers, url, b.mime, ".wav");
    } else {
      base64 = b.base64;
      defaultPath = suggestedFileName(headers, url, b.mime);
    }

    const path = await save({ title: "保存响应体", defaultPath });
    if (!path) return;
    if (text !== null) {
      await invoke("write_text_file", { path, content: text });
    } else {
      await invoke("write_binary_file", { path, base64Data: base64 });
    }
    savedBody.value = true;
    setTimeout(() => (savedBody.value = false), 1500);
    toast.push("响应体已保存", "success", 2000);
  } catch (e) {
    toast.push(`保存失败：${String(e)}`, "error", 5000);
  }
}

// ---- 响应头过滤 ----
const headerFilter = ref("");
const filteredHeaders = computed(() => {
  const q = headerFilter.value.trim().toLowerCase();
  if (!q) return props.response.headers;
  return props.response.headers.filter(
    ([k, v]) => k.toLowerCase().includes(q) || v.toLowerCase().includes(q)
  );
});

// ---- 同 URL 历史耗时趋势（响应状态条迷你折线） ----
/** 最近 10 次（旧→新）同 URL 请求的耗时；含当前这次 */
const trend = computed<number[]>(() => {
  const url = props.response.url;
  if (!url) return [];
  const points = historyStore.items
    .filter((h) => h.url === url && h.timeMs != null)
    .slice(0, 10)
    .map((h) => h.timeMs!)
    .reverse();
  return points;
});

const trendPoints = computed<string>(() => {
  if (trend.value.length < 2) return "";
  const w = 52;
  const h = 14;
  const max = Math.max(...trend.value);
  const min = Math.min(...trend.value);
  const span = max - min || 1;
  return trend.value
    .map((v, i) => {
      const x = (i / (trend.value.length - 1)) * w;
      const y = h - 2 - ((v - min) / span) * (h - 4);
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(" ");
});

const trendTitle = computed(() =>
  trend.value.length < 2
    ? ""
    : `最近 ${trend.value.length} 次该 URL 请求耗时（旧→新）：\n${trend.value
        .map((v, i) => `#${i + 1} ${formatTime(v)}`)
        .join("\n")}`
);
</script>

<template>
  <div class="h-full flex flex-col">
    <!-- 状态条 -->
    <div class="flex items-center gap-3 px-2.5 h-8 flex-shrink-0 border-b border-app-border text-xs bg-app-surface">
      <div class="flex items-center gap-1.5">
        <span
          class="px-1.5 py-0.5 rounded font-mono font-semibold text-[11px]"
          :class="statusPillColor(response.status)"
        >
          {{ response.status }}
        </span>
        <span class="font-mono text-[11px]" :class="statusColor(response.status)">
          {{ response.statusText }}
        </span>
      </div>
      <div class="flex items-center gap-1 text-app-muted">
        <Clock :size="11" class="text-app-muted/70" />
        <span class="font-mono text-app-text">{{ formatTime(response.timeMs) }}</span>
      </div>
      <div class="flex items-center gap-1 text-app-muted">
        <Weight :size="11" class="text-app-muted/70" />
        <span class="font-mono text-app-text">{{ formatBytes(response.size) }}</span>
      </div>

      <!-- 同 URL 耗时趋势（≥2 个点时显示，悬停查看各次数值） -->
      <svg
        v-if="trendPoints"
        :viewBox="`0 0 52 14`"
        class="h-3.5 w-[52px] flex-shrink-0"
        :title="trendTitle"
      >
        <polyline
          :points="trendPoints"
          fill="none"
          stroke="rgb(var(--accent-blue))"
          stroke-width="1.2"
          stroke-linejoin="round"
          stroke-linecap="round"
        />
      </svg>

      <div class="ml-auto flex items-center gap-0.5">
        <button
          class="icon-btn"
          :class="savedBody ? 'text-accent-green hover:text-accent-green' : ''"
          :title="savedBody ? '已保存' : '保存响应体到文件'"
          @click="saveBodyToFile"
        >
          <Check v-if="savedBody" :size="13" />
          <Save v-else :size="13" />
        </button>
        <button
          class="seg"
          :class="view === 'body' ? 'seg-active' : ''"
          @click="view = 'body'"
        >
          Body
        </button>
        <button
          class="seg"
          :class="view === 'headers' ? 'seg-active' : ''"
          @click="view = 'headers'"
        >
          Headers ({{ response.headers.length }})
        </button>
      </div>
    </div>

    <!-- Body -->
    <div v-if="view === 'body'" class="flex-1 min-h-0 flex flex-col">
      <!-- 工具栏：所有文本响应都显示（mime + 复制），Pretty/Raw 仅对可格式化的语言显示 -->
      <div
        v-if="response.body.kind === 'text'"
        class="flex items-center gap-1 px-3 py-1 border-b border-app-border/40 text-xs text-app-muted"
      >
        <template v-if="language !== 'text'">
          <button class="seg" :class="pretty ? 'seg-active' : ''" @click="pretty = true">
            Pretty
          </button>
          <button class="seg" :class="!pretty ? 'seg-active' : ''" @click="pretty = false">
            Raw
          </button>
        </template>
        <FileText :size="12" class="ml-1 text-app-muted/60" />
        <span class="ml-auto">{{ response.body.mime || "text/plain" }}</span>
        <button
          class="icon-btn flex-shrink-0"
          :class="copiedBody ? 'text-accent-green hover:text-accent-green' : ''"
          :title="copiedBody ? '已复制' : '复制响应体'"
          @click="copyBody"
        >
          <Check v-if="copiedBody" :size="13" />
          <Copy v-else :size="13" />
        </button>
      </div>

      <!-- 音频播放：容器格式直接播；裸 PCM（如智谱 TTS 默认输出）先补 WAV 头 -->
      <div v-if="audioPlan" class="flex-1 min-h-0 flex flex-col">
        <div
          class="flex items-center gap-2 px-3 py-1 border-b border-app-border/40 text-xs text-app-muted flex-wrap"
        >
          <Music :size="12" class="text-app-muted/60" />
          <span>{{ response.body.mime || "audio" }}</span>
          <span class="text-app-muted/40">·</span>
          <span>{{ audioPlan.mode === "pcm" ? "裸 PCM（已补 WAV 头）" : "音频容器" }}</span>
          <span v-if="audioDuration || pcmSeconds" class="text-app-muted/40">·</span>
          <span v-if="audioDuration || pcmSeconds">
            {{ formatTime(Math.round((audioDuration || pcmSeconds) * 1000)) }}
          </span>
        </div>

        <div class="flex-1 flex flex-col items-center justify-center gap-3 p-6">
          <audio
            v-if="audioUrl"
            :src="audioUrl"
            controls
            class="w-full max-w-lg"
            @loadedmetadata="onAudioMeta"
            @error="audioFailed = true"
          />
          <div v-else class="text-xs text-app-muted">音频数据为空</div>

          <div
            v-if="audioError || audioFailed"
            class="flex items-start gap-1.5 max-w-lg text-xs text-accent-orange text-left"
          >
            <AlertTriangle :size="13" class="flex-shrink-0 mt-0.5" />
            <span>
              {{ audioError || "播放器无法解码该音频。" }}
              可切换解析方式重试。
            </span>
          </div>

          <!-- 裸 PCM 参数：按实际编码调整后即时重新包装 -->
          <div
            v-if="audioPlan.mode === 'pcm'"
            class="flex items-center gap-x-3 gap-y-2 flex-wrap justify-center text-xs text-app-muted"
          >
            <label class="flex items-center gap-1.5">
              采样率
              <select v-model.number="pcmFormat.sampleRate" class="input h-6">
                <option v-for="r in SAMPLE_RATES" :key="r" :value="r">{{ r }} Hz</option>
              </select>
            </label>
            <label class="flex items-center gap-1.5">
              声道
              <select v-model.number="pcmFormat.channels" class="input h-6">
                <option :value="1">单声道</option>
                <option :value="2">双声道</option>
              </select>
            </label>
            <label class="flex items-center gap-1.5">
              位深
              <select v-model.number="pcmFormat.bitsPerSample" class="input h-6">
                <option :value="8">8 bit</option>
                <option :value="16">16 bit</option>
                <option :value="32">32 bit</option>
              </select>
            </label>
          </div>

          <button
            v-if="audioError || audioFailed || audioOverride"
            class="seg"
            @click="audioOverride = audioPlan.mode === 'pcm' ? 'container' : 'pcm'"
          >
            按{{ audioPlan.mode === "pcm" ? "容器格式" : "裸 PCM（补 WAV 头）" }}解析
          </button>
        </div>
      </div>

      <!-- 视频播放 -->
      <div v-else-if="videoMime" class="flex-1 min-h-0 flex items-center justify-center p-4 bg-black/40">
        <video v-if="videoUrl" :src="videoUrl" controls class="max-w-full max-h-full" />
      </div>

      <!-- 图片预览 -->
      <div
        v-else-if="isImage"
        class="flex-1 overflow-auto p-4 flex items-start justify-center bg-[repeating-conic-gradient(#333_0%_25%,#222_0%_50%)] bg-[length:16px_16px]"
      >
        <img :src="imgDataUrl" class="max-w-full max-h-full object-contain" alt="response image" />
      </div>

      <!-- 其它二进制：可存为文件、试探性按音频解析 -->
      <div
        v-else-if="response.body.kind === 'binary'"
        class="flex-1 flex items-center justify-center text-app-muted"
      >
        <div class="text-center">
          <ImageIcon :size="32" class="mx-auto opacity-40 mb-2" />
          <div class="text-sm">二进制响应 ({{ response.body.mime || "未知类型" }})</div>
          <div class="text-xs mt-1">{{ formatBytes(response.size) }}</div>
          <button class="seg mt-3 mx-auto" @click="audioOverride = 'pcm'">
            <Music :size="12" />
            尝试按音频播放
          </button>
        </div>
      </div>
      <!-- 文本 -->
      <div v-else class="flex-1 min-h-0">
        <div
          v-if="truncated"
          class="flex items-center gap-2 px-3 py-1.5 text-xs border-b border-app-border/40 bg-accent-orange/10 text-accent-orange"
        >
          <AlertTriangle :size="13" class="flex-shrink-0" />
          <span class="truncate">
            响应体过大（{{ formatBytes(response.size) }}），已截断为前 {{ MAX_DISPLAY_CHARS.toLocaleString() }} 字符
          </span>
          <button
            class="ml-auto flex-shrink-0 px-2 py-0.5 rounded bg-app-hover hover:brightness-110 text-app-text"
            @click="showFull = !showFull"
          >
            {{ showFull ? "恢复截断" : "显示完整" }}
          </button>
        </div>
        <MonacoEditor :model-value="displayText" :language="language" read-only />
      </div>
    </div>

    <!-- Headers -->
    <div v-else class="flex-1 min-h-0 flex flex-col">
      <!-- 过滤框：响应头多时快速定位 -->
      <div class="flex items-center px-3 h-8 gap-2 border-b border-app-border/40 flex-shrink-0">
        <Search :size="12" class="text-app-muted flex-shrink-0" />
        <input
          v-model="headerFilter"
          placeholder="过滤响应头（名称或值）…"
          spellcheck="false"
          class="flex-1 bg-transparent text-xs outline-none placeholder:text-app-muted/60"
        />
        <span class="text-[10px] text-app-muted flex-shrink-0">
          {{ filteredHeaders.length }}/{{ response.headers.length }}
        </span>
      </div>
      <div class="flex-1 min-h-0 overflow-auto">
      <table class="w-full text-sm">
        <thead class="sticky top-0 bg-app-surface">
          <tr class="border-b border-app-border text-left">
            <th class="px-3 py-1 font-medium text-xs text-app-muted font-mono w-1/3">Name</th>
            <th class="px-3 py-1 font-medium text-xs text-app-muted font-mono">Value</th>
            <th class="w-8" />
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="(h, i) in filteredHeaders"
            :key="i"
            class="border-b border-app-border/30 group hover:bg-app-hover/60"
          >
            <td class="px-3 py-1 font-mono text-xs text-accent-blue/80 align-top w-1/3">{{ h[0] }}</td>
            <td class="px-3 py-1 font-mono text-xs text-app-text break-all">{{ h[1] }}</td>
            <td class="px-1 py-1 w-8 text-right align-top">
              <button
                class="icon-btn opacity-0 group-hover:opacity-100 transition-opacity"
                :class="copiedHeader === h[0] ? 'text-accent-green opacity-100' : ''"
                title="复制响应头"
                @click="copyHeader(h[0], h[1])"
              >
                <Check v-if="copiedHeader === h[0]" :size="13" />
                <Copy v-else :size="13" />
              </button>
            </td>
          </tr>
        </tbody>
      </table>
      </div>
    </div>
  </div>
</template>

import { invoke } from "@tauri-apps/api/core";
import type { HttpRequestPayload, HttpResponsePayload } from "@/types";

/** 发起 HTTP 请求（可选 requestId：传入后可通过 cancel_http_request 终止在途请求） */
export function useInvoke() {
  const sendHttpRequest = (req: HttpRequestPayload, requestId?: string) =>
    invoke<HttpResponsePayload>("send_http_request", { req, requestId });

  return { sendHttpRequest };
}

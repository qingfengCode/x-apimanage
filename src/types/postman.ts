// Postman Collection v2.1 schema 的最小子集（够导入/导出用）
// 参考: https://schema.postman.com/json/collection/v2.1.0/collection.json

export interface PmCollection {
  info: {
    name: string;
    _postman_id?: string;
    description?: string;
    schema: string; // 固定 "https://schema.postman.com/.../v2.1.0/collection.json"
  };
  item: PmItem[];
  variable?: PmVariable[];
}

export interface PmItem {
  name: string;
  description?: string;
  // 子项（文件夹）或请求，二选一
  item?: PmItem[];
  request?: PmRequest;
  event?: PmEvent[]; // prerequest / test 脚本
}

export interface PmEvent {
  listen: "prerequest" | "test";
  script: {
    type?: string;
    exec: string[] | string; // 行数组或单字符串
  };
}

export interface PmRequest {
  method: string;
  header?: PmHeader[];
  url: PmUrl | string;
  body?: PmBody;
  auth?: PmAuth;
  description?: string;
}

export interface PmHeader {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

export interface PmUrl {
  raw?: string;
  protocol?: string;
  host?: string[];
  path?: string[];
  query?: PmQuery[];
  variable?: PmPathVariable[];
}

export interface PmQuery {
  key: string;
  value: string;
  disabled?: boolean;
  description?: string;
}

export interface PmPathVariable {
  key: string;
  value: string;
  description?: string;
}

export type PmBodyMode = "raw" | "urlencoded" | "formdata" | "file";

export interface PmBody {
  mode: PmBodyMode;
  raw?: string;
  urlencoded?: PmUrlEncoded[];
  formdata?: PmFormData[];
  file?: { content?: string; src?: string };
  options?: {
    raw?: { language?: string }; // json|text|javascript|html|xml
  };
}

export interface PmUrlEncoded {
  key: string;
  value: string;
  disabled?: boolean;
}

export interface PmFormData {
  key: string;
  /** file 类型条目通常没有 value（用 src 定位文件） */
  value?: string;
  type?: string; // "text" | "file"
  disabled?: boolean;
  src?: string | null;
}

export interface PmAuth {
  type: string;
  [k: string]: any;
}

export interface PmVariable {
  key: string;
  value: string;
  disabled?: boolean;
  type?: string;
}

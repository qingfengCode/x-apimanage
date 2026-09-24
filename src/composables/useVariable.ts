import type { KeyValue } from "@/types";

const VAR_RE = /\{\{\s*([\w.]+)\s*\}\}/g;

/**
 * 变量解析 composable：把 {{var}} 替换为环境变量值。
 * 这里不依赖 store，避免循环依赖；由调用方传入变量映射。
 */
export function useVariable() {
  /** 解析单个字符串 */
  function resolveString(input: string, vars: Map<string, string>): string {
    return input.replace(VAR_RE, (full, name: string) => {
      return vars.has(name) ? vars.get(name)! : full;
    });
  }

  /** 提取字符串中所有未解析的变量名 */
  function extractVars(input: string): string[] {
    const set = new Set<string>();
    let m: RegExpExecArray | null;
    const re = new RegExp(VAR_RE);
    while ((m = re.exec(input))) {
      set.add(m[1]);
    }
    return [...set];
  }

  /** 从 KeyValue[] 构建变量映射 */
  function buildVarMap(items: KeyValue[]): Map<string, string> {
    const map = new Map<string, string>();
    for (const kv of items) {
      if (kv.enabled && kv.key) map.set(kv.key, kv.value);
    }
    return map;
  }

  return { resolveString, extractVars, buildVarMap, VAR_RE };
}

import {
  newQuickJSWASMModuleFromVariant,
  newVariant,
  RELEASE_SYNC,
  shouldInterruptAfterDeadline,
} from "quickjs-emscripten";
import type { QuickJSContext, QuickJSHandle } from "quickjs-emscripten";
// 显式声明 wasm 资源地址：默认变体在运行时用 import.meta.url 相对定位 wasm，
// vite dev 预打包会把 import.meta.url 指到 /node_modules/.vite/deps/（该目录下
// 没有 wasm），请求 404 后被 SPA fallback 兜底返回 index.html，WebAssembly
// 校验魔数失败（expected magic word ... found 3c 21 64 6f = "<!do"）。
// ?url 在 dev 下由 vite 直接服务源文件、在 build 下产出带 hash 的静态资源，
// 两种环境都能拿到真正的 wasm。
import quickjsWasmUrl from "@jitl/quickjs-wasmfile-release-sync/wasm?url";
import type {
  HttpResponsePayload,
  ScriptRunResult,
  TestResult,
} from "@/types";

/** 脚本单次执行超时（毫秒）：QuickJS 在主线程同步执行，
 * 没有上限时 `while(true)` 这类死循环会永久冻结整个窗口 */
const SCRIPT_TIMEOUT_MS = 5_000;

function scriptErrorMsg(e: unknown): string {
  const raw = e as { message?: unknown };
  const msg = stringifyErr(e);
  if (
    msg.includes("interrupted") ||
    String(raw?.message ?? "").includes("interrupted")
  ) {
    return `脚本执行超时（超过 ${SCRIPT_TIMEOUT_MS / 1000}s，已强制中断）；请检查是否有死循环或耗时过长的同步操作`;
  }
  return msg;
}

// 模块级缓存：QuickJS 实例（wasm 较重，只加载一次）
// 用显式 async 函数包裹，便于捕获底层 emscripten 加载错误并给出可读提示
interface QuickJSModule {
  newContext(): QuickJSContext;
}
let qsPromise: Promise<QuickJSModule> | null = null;

/** 指定 wasmLocation 的 release-sync 变体（见顶部 ?url 说明） */
const qsVariant = newVariant(RELEASE_SYNC, { wasmLocation: quickjsWasmUrl });

async function qs(): Promise<QuickJSModule> {
  if (!qsPromise) {
    qsPromise = (async () => {
      try {
        const mod = await newQuickJSWASMModuleFromVariant(qsVariant);
        return mod as unknown as QuickJSModule;
      } catch (e) {
        // 清理让后续可重试
        qsPromise = null;
        throw new Error(
          `测试脚本引擎加载失败：${stringifyErr(e)}。` +
            `可能原因：wasm 资源（${quickjsWasmUrl}）加载失败。`
        );
      }
    })();
  }
  return qsPromise;
}

/** Pre-script 执行的上下文 */
export interface PreContext {
  /** 已合并的变量 map（环境变量 + 全局） */
  variables: Map<string, string>;
}

/** Test-script 执行的上下文 */
export interface TestContext {
  response: HttpResponsePayload;
  /** 当前变量（Pre 写入的会合并进来） */
  variables: Map<string, string>;
}

/**
 * 执行 pre-request 脚本（在请求发送前）。
 * 脚本可读变量、可 set 变量（影响本次请求的变量解析）。
 */
export async function runPreScript(code: string, ctx: PreContext): Promise<{
  variables: Record<string, string>;
  error?: string;
  logs: string[];
}> {
  if (!code.trim()) return { variables: mapToObj(ctx.variables), logs: [] };

  const logs: string[] = [];
  const setVars = new Map<string, string>(ctx.variables);

  const Qs = await qs();
  const vm = Qs.newContext();

  try {
    defineConsole(vm, logs);
    definePmForPre(vm, setVars);
    vm.runtime.setInterruptHandler(shouldInterruptAfterDeadline(Date.now() + SCRIPT_TIMEOUT_MS));
    const result = vm.evalCode(code);
    if (result.error) {
      const e = vm.dump(result.error);
      result.error.dispose();
      return { variables: mapToObj(setVars), logs, error: scriptErrorMsg(e) };
    }
    result.value.dispose();
    return { variables: mapToObj(setVars), logs };
  } finally {
    vm.runtime.removeInterruptHandler();
    vm.dispose();
  }
}

/**
 * 执行 test 脚本（在收到响应后）。
 * 脚本可断言 pm.test("name", fn)、读 pm.response、set 变量。
 */
export async function runTestScript(
  code: string,
  ctx: TestContext
): Promise<{ tests: TestResult[]; error?: string; logs: string[]; variables: Record<string, string> }> {
  if (!code.trim()) return { tests: [], logs: [], variables: mapToObj(ctx.variables) };

  const logs: string[] = [];
  const setVars = new Map<string, string>(ctx.variables);
  const tests: TestResult[] = [];

  const Qs = await qs();
  const vm = Qs.newContext();

  try {
    defineConsole(vm, logs);
    definePmForTest(vm, ctx.response, tests, setVars);
    vm.runtime.setInterruptHandler(shouldInterruptAfterDeadline(Date.now() + SCRIPT_TIMEOUT_MS));
    const result = vm.evalCode(code);
    if (result.error) {
      const e = vm.dump(result.error);
      result.error.dispose();
      return { tests, logs, error: scriptErrorMsg(e), variables: mapToObj(setVars) };
    }
    result.value.dispose();
    return { tests, logs, variables: mapToObj(setVars) };
  } finally {
    vm.runtime.removeInterruptHandler();
    vm.dispose();
  }
}

/** 一站式：跑 pre + test（test 需要响应）。返回合并结果 */
export async function runScripts(opts: {
  preScript: string;
  testScript: string;
  variables: Map<string, string>;
  response: HttpResponsePayload;
}): Promise<ScriptRunResult> {
  const out: ScriptRunResult = { variables: {}, tests: [], logs: [] };
  if (opts.preScript.trim()) {
    const r = await runPreScript(opts.preScript, { variables: opts.variables });
    out.preError = r.error;
    out.logs.push(...r.logs);
    out.variables = r.variables;
    // 合并 set 的变量进 test 用的 map
    for (const k of Object.keys(r.variables)) opts.variables.set(k, r.variables[k]);
  }
  if (opts.testScript.trim()) {
    const r = await runTestScript(opts.testScript, {
      response: opts.response,
      variables: opts.variables,
    });
    out.testError = r.error;
    out.tests = r.tests;
    out.logs.push(...r.logs);
    out.variables = { ...out.variables, ...r.variables };
  }
  return out;
}

// ============ 沙箱内 pm 对象定义 ============

function defineConsole(vm: QuickJSContext, logs: string[]) {
  const logFn = vm.newFunction("log", (...args) => {
    logs.push(args.map((a) => vm.dump(a)).join(" "));
  });
  const consoleObj = vm.newObject();
  vm.setProp(consoleObj, "log", logFn);
  logFn.dispose();
  vm.setProp(vm.global, "console", consoleObj);
  consoleObj.dispose();
}

/** Pre-script 的 pm：变量读写 */
function definePmForPre(vm: QuickJSContext, setVars: Map<string, string>) {
  const pm = vm.newObject();

  // pm.variables.get(key)
  const getFn = vm.newFunction("get", (keyHandle) => {
    const key = vm.getString(keyHandle);
    return vm.newString(setVars.get(key) ?? "");
  });
  const setFn = vm.newFunction("set", (keyHandle, valHandle) => {
    const key = vm.getString(keyHandle);
    const val = vm.getString(valHandle);
    setVars.set(key, val);
  });
  const unsetFn = vm.newFunction("unset", (keyHandle) => {
    setVars.delete(vm.getString(keyHandle));
  });
  const isSetFn = vm.newFunction("isSet", (keyHandle) => {
    return setVars.has(vm.getString(keyHandle)) ? vm.true : vm.false;
  });

  // pm.environment.get / set / unset（与 variables 同义，Postman 兼容）
  const env = vm.newObject();
  vm.setProp(env, "get", getFn);
  vm.setProp(env, "set", setFn);
  vm.setProp(env, "unset", unsetFn);
  vm.setProp(env, "isSet", isSetFn);
  vm.setProp(pm, "environment", env);

  const varsObj = vm.newObject();
  vm.setProp(varsObj, "get", getFn);
  vm.setProp(varsObj, "set", setFn);
  vm.setProp(varsObj, "unset", unsetFn);
  vm.setProp(varsObj, "isSet", isSetFn);
  vm.setProp(pm, "variables", varsObj);

  // pm.test 空实现（pre 里一般不用）
  const testFn = vm.newFunction("test", () => vm.undefined);
  vm.setProp(pm, "test", testFn);

  vm.setProp(vm.global, "pm", pm);
  // dispose 句柄
  getFn.dispose();
  setFn.dispose();
  unsetFn.dispose();
  isSetFn.dispose();
  env.dispose();
  varsObj.dispose();
  testFn.dispose();
  pm.dispose();
}

/** Test-script 的 pm：response + test + expect + 变量 */
function definePmForTest(
  vm: QuickJSContext,
  response: HttpResponsePayload,
  tests: TestResult[],
  setVars: Map<string, string>
) {
  // pm.response（json/text/code/headers/responseTime + to.have.status/header）
  const respObj = buildResponseObject(vm, response);

  // 环境变量 host 函数
  const getFn = vm.newFunction("__getVar", (keyHandle) => {
    return vm.newString(setVars.get(vm.getString(keyHandle)) ?? "");
  });
  const setFn = vm.newFunction("__setVar", (keyHandle, valHandle) => {
    setVars.set(vm.getString(keyHandle), vm.getString(valHandle));
    return vm.undefined;
  });
  const unsetFn = vm.newFunction("__unsetVar", (keyHandle) => {
    setVars.delete(vm.getString(keyHandle));
    return vm.undefined;
  });
  const isSetFn = vm.newFunction("__isSetVar", (keyHandle) => {
    return setVars.has(vm.getString(keyHandle)) ? vm.true : vm.false;
  });

  // 测试断言记录原语
  const recordAssertFn = vm.newFunction("__recordAssert", (nameHandle, passedHandle, detailHandle) => {
    const name = vm.getString(nameHandle);
    // passed 由 bootstrap 传 1/0（number）或 true/false（bool），统一按真值判断
    const passedVal = vm.dump(passedHandle);
    const passed = passedVal === 1 || passedVal === true;
    const detail = vm.getString(detailHandle);
    tests.push({ name, passed, error: passed ? undefined : detail });
    return vm.undefined;
  });

  // 注入所有原语到全局
  vm.setProp(vm.global, "__getVar", getFn);
  vm.setProp(vm.global, "__setVar", setFn);
  vm.setProp(vm.global, "__unsetVar", unsetFn);
  vm.setProp(vm.global, "__isSetVar", isSetFn);
  vm.setProp(vm.global, "__recordAssert", recordAssertFn);

  // 在沙箱内用纯 JS 定义 pm / expect / test / chai-like 断言链
  // 这样所有对象/循环引用都是 QuickJS 原生 JS，GC 能正常回收，避免 host handle 泄漏
  const pmBootstrap = buildPmBootstrap(response);
  const r = vm.evalCode(pmBootstrap);
  if (r.error) {
    r.error.dispose();
  } else {
    r.value.dispose();
  }

  // 把 pm.response 句柄挂到 pm.response，并调用 bootstrap 提供的 __patchResponse
  // 补上 to.have.status / to.have.header（在 JS 侧定义，避免 native 句柄循环引用）
  const pmHandle = vm.getProp(vm.global, "pm");
  vm.setProp(pmHandle, "response", respObj);
  const patchFn = vm.getProp(pmHandle, "__patchResponse");
  const patchRes = vm.callFunction(patchFn, vm.undefined, respObj);
  if (patchRes.error) patchRes.error.dispose();
  else patchRes.value.dispose();
  patchFn.dispose();
  pmHandle.dispose();

  // 清理 host 句柄
  respObj.dispose();
  getFn.dispose();
  setFn.dispose();
  unsetFn.dispose();
  isSetFn.dispose();
  recordAssertFn.dispose();
}

/**
 * 生成沙箱内 pm/expect/test 的纯 JS 定义代码。
 * 所有断言逻辑都在 JS 内实现，host 仅通过 __recordAssert / __getVar 等回调交互。
 */
function buildPmBootstrap(response: HttpResponsePayload): string {
  // response 信息以字面量形式注入（供 to.have.status 等使用）
  const respLiteral = JSON.stringify({
    status: response.status,
    headers: Object.fromEntries(response.headers.map(([k, v]) => [k.toLowerCase(), v])),
  });
  return `
  (function () {
    // 处于 pm.test(name, fn) 内时，断言先聚合到测试上下文（__testFail*），
    // 由 pm.test 结束时统一记一条以 name 命名的结果，避免与内部 expect 双重计数
    var __inTest = 0;
    var __testFail = false;
    var __testFailDetail = "";
    var __assert = function (label, ok, detail) {
      if (__inTest > 0) {
        if (!ok && !__testFail) {
          __testFail = true;
          __testFailDetail = detail || label;
        }
        return;
      }
      globalThis.__recordAssert(label, ok ? 1 : 0, detail || "");
    };
    // ---- expect / chai-like ----
    var chainWords = { to: 1, be: 1, been: 1, is: 1, that: 1, which: 1, have: 1, has: 1, with: 1, at: 1, of: 1, same: 1, should: 1, not: 1, deep: 1, an: 0, a: 0 };
    function makeAssertion(actual, negated) {
      var a = {
        _neg: !!negated,
        _eval: function (cond, label, detail) {
          var ok = this._neg ? !cond : !!cond;
          __assert(label, ok, this._neg ? "(negated) " + (detail || label) : detail);
          return this;
        }
      };
      // 链式空词：返回 this（同义）
      ["to","be","been","is","that","which","have","has","with","at","of","same","should","deep"].forEach(function (w) {
        Object.defineProperty(a, w, { get: function () { return makeAssertion(actual, this._neg); }, configurable: true });
      });
      // not
      Object.defineProperty(a, "not", { get: function () { return makeAssertion(actual, !this._neg); }, configurable: true });
      // equal / equals / eql
      a.equal = function (v) { return this._eval(actual === v, "expect.equal", JSON.stringify(actual) + " !== " + JSON.stringify(v)); };
      a.equals = a.equal;
      a.eq = a.equal;
      a.eql = function (v) {
        var ok = deepEq(actual, v);
        return this._eval(ok, "expect.eql", JSON.stringify(actual) + " !== " + JSON.stringify(v));
      };
      a.deep = a.eql;
      // above / below / greaterThan / lessThan
      a.above = function (n) { return this._eval(Number(actual) > n, "expect.above", actual + " <= " + n); };
      a.greaterThan = a.above;
      a.gt = a.above;
      a.below = function (n) { return this._eval(Number(actual) < n, "expect.below", actual + " >= " + n); };
      a.lessThan = a.below;
      a.lt = a.below;
      a.least = function (n) { return this._eval(Number(actual) >= n, "expect.at.least", actual + " < " + n); };
      a.most = function (n) { return this._eval(Number(actual) <= n, "expect.at.most", actual + " > " + n); };
      // true / false / ok
      Object.defineProperty(a, "true", { get: function () { return this._eval(actual === true || actual === 1 || actual === "true", "expect.true", JSON.stringify(actual)); }, configurable: true });
      Object.defineProperty(a, "false", { get: function () { return this._eval(!actual, "expect.false", JSON.stringify(actual)); }, configurable: true });
      Object.defineProperty(a, "ok", { get: function () { return this._eval(!!actual, "expect.ok", JSON.stringify(actual)); }, configurable: true });
      Object.defineProperty(a, "exist", { get: function () { return this._eval(actual !== null && actual !== undefined, "expect.exist", JSON.stringify(actual)); }, configurable: true });
      Object.defineProperty(a, "defined", { get: function () { return this._eval(actual !== undefined, "expect.defined", JSON.stringify(actual)); }, configurable: true });
      Object.defineProperty(a, "null", { get: function () { return this._eval(actual === null, "expect.null", JSON.stringify(actual)); }, configurable: true });
      // an / a (type)
      a.an = function (t) {
        var ok;
        if (t === "array") ok = Array.isArray(actual);
        else if (t === "object") ok = actual !== null && typeof actual === "object" && !Array.isArray(actual);
        else ok = typeof actual === t;
        return this._eval(ok, "expect.an(" + t + ")", typeof actual);
      };
      a.a = a.an;
      // include / contain
      a.include = function (m) {
        var ok;
        if (Array.isArray(actual)) ok = actual.some(function (x) { return deepEq(x, m); });
        else if (typeof actual === "string") ok = actual.indexOf(String(m)) >= 0;
        else if (actual && typeof actual === "object") ok = m in actual;
        else ok = false;
        return this._eval(ok, "expect.include", JSON.stringify(actual) + " 不含 " + JSON.stringify(m));
      };
      a.contain = a.include; a.includes = a.include; a.contains = a.include;
      // property
      a.property = function (k, v) {
        var has = actual && typeof actual === "object" && k in actual;
        if (!has) return this._eval(false, "expect.property(" + k + ")", "missing " + k);
        if (arguments.length > 1) return this._eval(deepEq(actual[k], v), "expect.property(" + k + ")", JSON.stringify(actual[k]) + " !== " + JSON.stringify(v));
        return this._eval(true, "expect.property(" + k + ")", "");
      };
      // match (regex source as string)
      a.match = function (re) {
        var src = (re && typeof re === "object" && re.source) ? re.source : String(re);
        var fl = (re && typeof re === "object" && re.flags) ? re.flags : "";
        var ok;
        try { ok = new RegExp(src, fl).test(String(actual)); } catch (e) { ok = false; }
        return this._eval(ok, "expect.match", JSON.stringify(actual) + " !~ " + src);
      };
      a.string = function (s) { return this._eval(String(actual).indexOf(s) >= 0, "expect.string", ""); };
      // length
      Object.defineProperty(a, "empty", { get: function () {
        var len = actual == null ? 0 : (actual.length !== undefined ? actual.length : Object.keys(actual).length);
        return this._eval(len === 0, "expect.empty", "length=" + len);
      }, configurable: true });
      a.lengthOf = function (n) {
        var len = actual == null ? 0 : (actual.length !== undefined ? actual.length : Object.keys(actual).length);
        return this._eval(len === n, "expect.lengthOf", "length=" + len + " expected " + n);
      };
      return a;
    }
    function deepEq(a, b) {
      if (a === b) return true;
      if (typeof a !== typeof b) return false;
      if (a && b && typeof a === "object") {
        var ka = Object.keys(a), kb = Object.keys(b);
        if (ka.length !== kb.length) return false;
        for (var i = 0; i < ka.length; i++) if (!deepEq(a[ka[i]], b[ka[i]])) return false;
        return true;
      }
      return false;
    }
    globalThis.expect = function (actual) { return makeAssertion(actual, false); };

    // ---- pm 对象 ----
    var _respInfo = ${respLiteral};
    var env = {
      get: function (k) { return globalThis.__getVar(k); },
      set: function (k, v) { globalThis.__setVar(k, String(v)); },
      unset: function (k) { globalThis.__unsetVar(k); },
      isSet: function (k) { return globalThis.__isSetVar(k) === true || globalThis.__isSetVar(k) === 1; }
    };
    var pm = {
      environment: env,
      variables: env,
      response: null, // 由 host 注入真实对象
      test: function (name, fn) {
        __inTest++;
        __testFail = false;
        __testFailDetail = "";
        try {
          var r = fn();
          if (r && typeof r.then === "function") {
            // 异步不支持，标记失败（按失败处理以提醒用户）
            __assert(name, false, "异步断言暂不支持");
          } else if (__testFail) {
            // 内部 expect 断言失败 → 整条 test 记为失败（只记这一条）
            __assert(name, false, __testFailDetail);
          } else {
            __assert(name, true, "");
          }
        } catch (e) {
          __assert(name, false, (e && e.message) ? String(e.message) : String(e));
        } finally {
          __inTest--;
        }
      },
      expect: globalThis.expect
    };
    // response.to.have.status / header
    // （response.json/text/code 等由 host 注入的 pm.response 提供，这里只补 to.have）
    pm.__patchResponse = function (respObj) {
      var to = { have: {
        status: function (code) { if (code !== _respInfo.status) throw new Error("expected status " + code + " got " + _respInfo.status); },
        header: function (name) { if (!(String(name).toLowerCase() in _respInfo.headers)) throw new Error("missing header " + name); }
      }};
      try {
        Object.defineProperty(respObj, "to", { value: to, configurable: true });
        Object.defineProperty(respObj, "have", { value: to.have, configurable: true });
      } catch (e) {}
    };
    globalThis.pm = pm;
  })();
  // host 会在注入真实 pm.response 后调用
  globalThis.__pmReady = true;
  `;
}

/** 构造 pm.response 对象（json/text/status/code/headers） */
function buildResponseObject(vm: QuickJSContext, response: HttpResponsePayload): QuickJSHandle {
  const obj = vm.newObject();
  // code / status
  vm.setProp(obj, "code", vm.newNumber(response.status));
  vm.setProp(obj, "status", vm.newString(response.statusText));

  // text()
  const text = response.body.kind === "text" ? response.body.text : "";
  const textFn = vm.newFunction("text", () => vm.newString(text));
  vm.setProp(obj, "text", textFn);
  textFn.dispose();

  // json()
  const jsonFn = vm.newFunction("json", () => {
    if (response.body.kind !== "text") return vm.null;
    try {
      // 用 JSON.stringify 生成合法的 JS 字符串字面量（含换行/引号等全部转义），
      // 在沙箱内解析，确保返回真正的 JS 对象
      const r = vm.evalCode(`JSON.parse(${JSON.stringify(response.body.text)})`);
      if (r.error) {
        r.error.dispose();
        return vm.null;
      }
      return r.value;
    } catch {
      return vm.null;
    }
  });
  vm.setProp(obj, "json", jsonFn);
  jsonFn.dispose();

  // headers()
  const headersObj = vm.newObject();
  const headerMap = new Map<string, string>();
  for (const [k, v] of response.headers) {
    const lk = k.toLowerCase();
    headerMap.set(lk, v);
    // 取小写 key 方便访问
    vm.setProp(headersObj, lk, vm.newString(v));
  }
  vm.setProp(obj, "headers", headersObj);
  headersObj.dispose();

  // responseTime
  vm.setProp(obj, "responseTime", vm.newNumber(response.timeMs));

  return obj;
}

// ============ 工具 ============

function mapToObj(m: Map<string, string>): Record<string, string> {
  const o: Record<string, string> = {};
  for (const [k, v] of m) o[k] = v;
  return o;
}

function stringifyErr(e: any): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as any).message);
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

function deepEqual(a: any, b: any): boolean {
  if (a === b) return true;
  if (typeof a !== typeof b) return false;
  if (a && b && typeof a === "object") {
    const ka = Object.keys(a);
    const kb = Object.keys(b);
    if (ka.length !== kb.length) return false;
    return ka.every((k) => deepEqual(a[k], b[k]));
  }
  return false;
}

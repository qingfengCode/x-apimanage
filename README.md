# x-apimanage

一个基于 **Tauri 2 + Vue 3** 的本地优先（local-first）API 调试平台，类似 Postman。

## 功能（MVP）

- ✅ HTTP 请求构造器：GET/POST/PUT/DELETE/PATCH/HEAD/OPTIONS
- ✅ Params / Headers / Body（Raw / x-www-form-urlencoded / Multipart）
- ✅ 响应查看：状态码、耗时、大小、响应头、Body（Monaco 编辑器，JSON 自动美化、图片预览、音频/视频播放、保存到文件）
- ✅ 集合（Collection）树形管理：集合 / 文件夹 / 请求，CRUD + 持久化
- ✅ 环境变量：多环境、`{{var}}` 变量解析（URL / Header / Body 通用）
- ✅ 历史记录：自动记录每次请求
- ✅ 多 Tab 请求页
- ✅ **导入/导出 Postman Collection v2.1**（保留文件夹结构、Body、Headers、脚本）
- ✅ **Pre/Post 测试脚本**（QuickJS 沙箱，支持 `pm.test` / `pm.expect` / `pm.response` / `pm.environment`）
- ✅ **Mock 服务**（内置本地 HTTP server，支持路径参数 `:id`、通配 `**`、延迟、状态码、自定义响应头）
- ✅ **易用性打磨**：
  - 复制为 cURL / 复制解析后 URL
  - URL 变量解析实时预览（`{{baseUrl}}` → 实际值）
  - 集合树搜索（按名称/URL/方法）
  - 历史记录完整快照恢复（一键重发，含 Body/Headers/脚本）+ 搜索过滤
  - 请求/集合/文件夹右键重命名（原生输入框）
  - 关闭未保存 Tab 二次确认（防丢数据）
  - GET/HEAD 自动隐藏 Body
  - 请求/响应区可拖拽分栏
  - 文件型响应就地预览：音频可播放、视频可播放、图片可查看，一键保存（文件名优先取 `Content-Disposition`）
- 🗓️ 规划中：批量运行 (Runner)、团队同步

## 技术栈

| 层 | 选型 |
|---|---|
| 前端 | Vue 3 + TypeScript + Vite 6 |
| UI | Tailwind CSS 3 + Headless UI Vue + lucide-vue-next |
| 状态管理 | Pinia |
| 代码编辑器 | Monaco Editor（Vite `?worker`） |
| 后端 | Rust + Tauri 2 |
| HTTP 引擎 | Rust `reqwest`（rustls + cookies，无 CORS 限制） |
| 存储 | `rusqlite`（bundled SQLite，无需系统安装） |
| Mock 服务 | Rust `axum` + `tokio`（本地 HTTP server） |
| 测试脚本 | 前端 `quickjs-emscripten`（JS 沙箱，无 `eval` 安全风险） |
| Postman 互操作 | v2.1 Collection 导入/导出 |

## 目录结构

```
x-apimanage/
├── src/                      # 前端
│   ├── components/           # layout / request / response / collection / environment / editor / common
│   ├── composables/          # useVariable ({{}} 解析) / useInvoke
│   ├── stores/               # tab / collection / environment / history (Pinia)
│   ├── types/                # 与 Rust 共享的 TS 类型
│   └── utils/                # id / format / mime
├── src-tauri/
│   ├── src/
│   │   ├── http/             # engine.rs (reqwest) + model.rs
│   │   ├── db/               # schema.rs (migration) + repos/
│   │   ├── commands/         # Tauri invoke 入口
│   │   └── error.rs
│   └── tauri.conf.json
└── build-msvc.cmd            # Windows 构建辅助脚本（见下）
```

## 开发

### 1. 安装依赖

```bash
npm install
```

### 2. 开发热重载

```bash
npm run tauri dev
```

> Windows 下若链接报 `unresolved external symbol ___chkstk_ms`，请改用本项目自带的
> **`build-msvc.cmd`** 脚本（见下方"Windows 构建注意事项"）。

### 3. 打包发布

```bash
npm run tauri build
```

产物位于 `src-tauri/target/release/bundle/`。

## Windows 构建注意事项 ⚠️

本机 PATH 中存在 TDM-GCC（`C:\mypc\TDM-GCC-64\bin\gcc.exe`）。Rust 的 `cc` crate
在 `cargo build` 时会优先用 PATH 中的 gcc 编译 `libsqlite3-sys`，产生 MinGW 专有的
`__chkstk_ms` 符号；而 Tauri 的 MSVC 目标最终用 `link.exe` 链接，找不到该符号，报：

```
LNK2019: unresolved external symbol ___chkstk_ms
```

**解决**：使用仓库根目录的 `build-msvc.cmd`，它会：

1. 加载 Visual Studio 2022 的 MSVC x64 环境（`vcvarsall.bat`）；
2. 从 PATH 移除 TDM-GCC；
3. 强制 `CC_x86_64_pc_windows_msvc=cl.exe`；
4. 执行 `npx tauri build`。

```cmd
:: 直接双击，或
cmd /c build-msvc.cmd
```

> 开发模式可把脚本最后一行的 `tauri build` 改为 `tauri dev`。
> 根本上修复建议：把 TDM-GCC 从系统 PATH 移除（若不需要它做其他事）。

## 数据存储

数据保存在系统应用数据目录的 `x-apimanage.db`（SQLite）：

- Windows: `C:\Users\<你>\AppData\Roaming\com.xapimanage.app\x-apimanage.db`

首次启动只自动建表，**不写入任何示例/模拟数据**——环境、集合、请求全部由你创建真实数据
（历史旧版本写入的出厂演示集合/示例环境会在升级时自动清理，改动过的会保留）。

## 快捷操作

| 快捷键 | 功能 |
|---|---|
| `Ctrl/Cmd + T` | 新建请求 Tab |
| `Ctrl/Cmd + W` | 关闭当前 Tab |
| `Ctrl/Cmd + S` | 保存当前请求（已关联集合则直接存） |
| `Ctrl/Cmd + Enter` | 发送当前请求 |
| `Ctrl/Cmd + Tab` | 下一个 Tab |
| `Ctrl/Cmd + Shift + Tab` | 上一个 Tab |

- **新建请求 Tab**：左侧栏「新建请求」或从集合点击请求
- **变量引用**：在 URL / Header / Body 中使用 `{{baseUrl}}`，发送前自动用当前环境的值替换
- **Tab 拖拽**：按住 Tab 标签拖动可重新排序

## 文件型响应（音频 / 视频 / 图片）

响应若是文件而非文本（`Content-Type` 为 `audio/*`、`video/*`、`image/*` 等），响应区会就地给出对应查看器：

- **音频**：直接播放（wav / mp3 / ogg / opus / flac / m4a / aac / webm）。
- **裸 PCM**：智谱 GLM-TTS 等接口的 `response_format=pcm` 返回的是没有容器头的裸采样流，浏览器无法直接解码。
  这类响应会自动补 44 字节 WAV 头后播放，采样率（默认 24000Hz）/ 声道 / 位深可在播放器下方调整，改完即时重新包装。
- **音频/视频一律走 Blob URL**，不把 base64 塞进 DOM 属性，大响应不会拖慢界面。
- **解析方式兜底**：服务端 `Content-Type` 与实际内容不符时（例如声明 `audio/wav` 却发裸 PCM），
  播放失败后可一键「按裸 PCM / 按容器格式」切换解析方式重试；`application/octet-stream` 响应
  可点「尝试按音频播放」。
- **保存到文件**：状态栏保存按钮按响应类型推断扩展名，文件名优先取 `Content-Disposition`
  （含 RFC 5987 中文名）；裸 PCM 会以补好 WAV 头的形式导出，存下来即可直接播放。

以智谱文本转语音为例：

```
POST https://open.bigmodel.cn/api/paas/v4/audio/speech
Authorization: Bearer {{apiKey}}
Content-Type: application/json

{ "model": "glm-tts", "input": "你好，今天天气怎么样", "voice": "tongtong", "response_format": "wav" }
```

发送后响应区即为音频播放器。`response_format` 省略时服务端默认返回裸 PCM，同样可直接播放。

## Postman 导入/导出

- **导入**：左侧 Collections 标题栏 → 下载图标 → 「导入 Postman Collection」→ 选择 `.json` 文件。
  支持文件夹结构、Body、Headers、Pre/Test 脚本，会作为新集合创建。
- **导出**：同对话框「导出」→ 选择集合 → 选保存路径，生成 v2.1 兼容 JSON。

## 测试脚本（Pre/Post）

在请求编辑区切到 **Scripts** Tab：

- **Pre-Request Script**（发送前执行）：可读写变量
  ```js
  const user = pm.environment.get("user");
  pm.environment.set("token", "auto-" + user);
  ```
- **Tests**（收到响应后执行）：断言 + 提取变量
  ```js
  pm.test("状态码 200", () => {
    pm.expect(pm.response.code).to.equal(200);
  });
  pm.test("响应体有 id", () => {
    const body = pm.response.json();
    pm.expect(body).to.have.property("id");
  });
  ```

支持的 pm API：`pm.environment.get/set`、`pm.variables.get/set`、`pm.response.code/status/text()/json()/headers/responseTime`、`pm.test(name, fn)`、`pm.expect(v).to.equal/eql/above/below/true`。脚本在 QuickJS 沙箱中运行，无法访问 DOM / fetch，安全隔离。结果展示在响应区下方的 Test Results 面板。

## Mock 服务

左侧栏切到 **Mock** Tab：

1. 点击「启动」（可填端口，留空自动分配），获得 `http://127.0.0.1:<port>`。
2. 「新建」添加路由，或「从集合生成」一键把现有集合的请求路径批量生成 mock。
3. 路由支持：
   - 方法：`GET/POST/PUT/DELETE/PATCH/ANY`（ANY 匹配任意方法）
   - 路径：精确匹配、`:param`（单段占位）、`**`（通配剩余）
   - 状态码、自定义响应头、延迟（ms）
4. 编辑路由响应体后自动热更新到运行中的 server（无需重启）。
5. 示例：路由 `GET /api/users/:id`，访问 `http://127.0.0.1:<port>/api/users/123` 即返回。

## AI 助手

顶部 Tab 栏右侧 **AI** 按钮（或 `Ctrl+I`）打开右侧聊天面板：

1. 首次使用点击面板右上角 ⚙ 填写 **OpenAI 兼容**模型服务（Base URL / API Key / 模型名），兼容 DeepSeek、GLM、Kimi、OpenAI、本地 Ollama 等；「测试连接」可验证。
2. 回复**流式输出**（打字机效果），可随时点 ■ 停止生成；支持**多会话**：头部历史按钮查看/切换/删除历史会话，会话存储在本地 SQLite，重启不丢。
3. AI 可执行真实工具（带工具调用循环）：
   - **写 API 调试配置**：`create_request` 直接在集合中创建可执行的请求
   - **调试接口**：`send_request` 实际发送请求并把响应喂回模型迭代
   - **写 API 文档 / 产品文档**：`save_document` 生成 Markdown 文档
4. 「附带当前请求上下文」勾选后，AI 能看到当前 Tab 的请求详情。
5. 生成的文档在左侧 **Docs** 页签管理：预览 / 编辑（Monaco）/ 复制 / 导出 .md / 删除。

## MCP 服务

顶部 Tab 栏 **MCP** 按钮（AI 按钮旁）打开 MCP 面板：

1. 启用后监听 `http://127.0.0.1:8765/mcp`（端口可改），随应用启动自动拉起。
2. 面板可**生成/吊销访问密钥**：生成后服务绑定 0.0.0.0，局域网设备凭密钥访问；
   面板提供**一键复制客户端配置 JSON**（含地址与密钥，形如
   `{"mcpServers":{"x-apimanage":{"url":"http://192.168.x.x:8765/mcp","headers":{"Authorization":"Bearer ..."}}}}`），
   直接粘贴进 Claude Code / Cursor 的 MCP 配置即可。
2. 在 Claude Code / Cursor 等支持 MCP 的客户端中添加 HTTP MCP 服务器，地址填上述 URL。
3. 暴露的工具与内置 AI 助手完全一致：
   `list_collections` / `list_requests` / `create_request`（写调试配置）/
   `send_request`（真实调试）/ `save_document`（写 API 文档 / 产品文档）/ `list_documents`
4. 同时暴露 **resources**（外部 AI 可直接读取应用数据）：
   - `xapimanage://collections` —— 全部集合与请求树（JSON）
   - `xapimanage://history` —— 最近 100 条请求历史
   - `xapimanage://documents/{id}` —— 每篇文档（Markdown，随文档列表动态注册）
   - `xapimanage://requests/{id}` —— 请求完整调试配置（URI 模板）
5. 协议：MCP Streamable HTTP（无状态 JSON-RPC 2.0），支持 tools + resources 能力。

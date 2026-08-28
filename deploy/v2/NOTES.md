# v2 Web 客户端开发经验与踩坑记录

本文记录 v2（用仓库**当前** `flutter/` 树构建 Web 客户端）开发过程中**非显而易见**的技术发现。v1 时代的经验（工具链位腐烂、hbbr loopback、relay 配对、headless 调试方法论等）见 [../v1/web/NOTES.md](../v1/web/NOTES.md)，本文不重复，只写 v2 新踩的坑。

## 一、最重要的架构发现：当前 flutter 树**本来就能编译 Web**

v1 NOTES 里写过「不要试图在当前 master 上补齐 v2——那是重写整个 JS 协议栈」。实际做下来发现这个判断**过于悲观**了：

- 上游虽然 2025-07 删了 `flutter/web/` 并加进 `.gitignore`，但 `flutter/lib` 里的 **Dart 侧 Web 支持完整保留**：
  - 条件导入（`if (dart.library.html)`）遍布 `main.dart`、`common.dart`、`platform_model.dart` 等，桌面/移动/Web 三路径都在；
  - `lib/web/bridge.dart`（`RustdeskImpl`）是完整的桥接 shim，`lib/models/web_model.dart` 是 Web 版 `PlatformFFI`；
  - `main.dart` 的 `home:` 直接有 `isWeb ? WebHomePage() : ...`，标题都是 `'... Web Client V2 (Preview)'`。
- 实测 `flutter build web --release`（Flutter 3.24.5）**一次通过，Dart 代码零改动**。`dart:io` 无条件 import、`window_manager` 等桌面插件在 Web 编译下都能过（它们的 Dart 代码可编译，原生实现运行时才缺位）。
- 真正缺的只有两样：`flutter/web/` 目录（index.html 等）和 **JS 协议栈**（上游 v2 的 JS 核心从未开源）。

所以 v2 的实际工作量 = 补 `flutter/web/` + 写一个 JS 协议栈，而不是「重写一切」。

### JS 协议栈不是从零发明

RustDesk 的线上协议由**服务端/被控端**（hbbs/hbbr、RustDesk 客户端）定义，任何兼容实现都必须走同样的步骤：rendezvous（PunchHoleRequest）→ relay（RequestRelay，按 uuid 配对）→ signed_id/公钥交换 → xsalsa20-poly1305 secretbox → Hash 挑战登录 → 视频帧。这些步骤在 `src/client.rs`、`src/server.rs`、`src/rendezvous_mediator.rs` 里都有权威实现可参考。v2 的 JS 栈（`flutter/web/js/src/`）是**对照当前协议写的新 TypeScript 实现**，关键对齐点：

- **协议代码用当前树生成**：`ts_proto.py` 对当前 `libs/hbb_common/protos/{message,rendezvous}.proto` 跑 ts-proto；`gen_js_from_hbb.py` 从当前 `src/lang/*`、`src/client.rs`、`Cargo.toml` 生成语言表/键映射/版本号。**v2 的协议跟随当前树自动演进**，这是它和 v1（冻结在 1.2.4 协议）的本质区别。
- 相比 v1 时代 proto 的漂移要适配：`LoginRequest` 新增 `session_id`/`version`/`my_platform`/`hwid`/`avatar`，`Auth2FA` 新增 `hwid`，`Misc.selected_sid` 变数字，`change_display_resolution` 取代废弃的 `change_resolution`，`VideoFrame` 多了 `vp8s`/`av1s` 等。

## 二、桥接面（bridge.dart）对齐：Dart ↔ JS 的契约

Dart 侧通过 `js.context.callMethod('setByName'/'getByName', ...)` 调 JS，JS 通过 `window.onGlobalEvent/onRgba/onRegisteredEvent/...` 回调 Dart。把当前 `bridge.dart` 全部读一遍、列出完整命令面是必须的——它和 v1 的 JS 栈有多处**命名漂移**，直接复用 v1 的 JS 会静默失败：

| 当前 bridge.dart | v1 JS 栈 | 后果 |
|---|---|---|
| `getByName('app-name')` | `app_name` | 应用名取不到 |
| `setByName('remove_peer')` | `remove` | 删除最近会话无效 |
| 聊天事件 `chat_client_mode` | `chat` | 收不到聊天消息 |
| `send_2fa` 带 `{code, trust_this_device}` | 裸 `code` | 2FA 字段缺失 |
| `web_model.init()` **等待** `window.onInitFinished()` | v1 的 `window.init` 从不调用它 | **App 启动即挂起**（最容易漏的一个） |

教训：**先完整列出 Dart 侧的 key/事件清单，再实现 JS 侧，最后对照 `model.dart` 的 `startEventListener` 核对每个事件名**。

## 三、构建与产物

1. **`flutter build web` 会把 `web/` 整个复制进产物**——包括 `js/node_modules`（98MB）和 `js/src`（生成的 proto）。`deploy/v2/web/build-web-client.sh` 在收集阶段裁剪，只留 `js/dist`（143MB → 34MB）。
2. **Flutter 3.24 用 `flutter_bootstrap.js`** 取代 3.19 时代的 service-worker 内联加载器；`index.html` 里 `<script src="flutter_bootstrap.js" async>` 即可，加载动画 div 会被 Flutter 画布盖住，无需手动移除。
3. **Service worker 缓存坑**：迭代测试时浏览器拿到的是旧资源（`flutter_service_worker.js` 按版本缓存）。测试务必「Bypass for network」/注销 SW/用全新 profile，否则会以为修复没生效（我们因此误判过一次回归）。
4. **`gen_js_from_hbb.py` 的 `check_if_retry` 文本重写法很脆**：v1 的生成器把 Rust 源码做字符串替换（`contains`→`indexOf` 之类），当前 `client.rs` 的 `check_if_retry` 加了 `use_ws()` 调用后直接生成出未定义符号、且逻辑被反转。v2 改成在生成器里**手写该函数的 JS 移植**（Web 端恒走 WS，`use_ws()` 恒为 true）。
5. **ogv.js 的 worker 代理表只覆盖非 MT 类**：`OGVDecoderVideoVP9SIMDW`/`VP9W`/`VP8W` 有 worker 代理，但 **VP8 没有 SIMD 构建**——请求 `OGVDecoderVideoVP8SIMDW` 会抛 `Requested worker for class with no proxy`（异步抛、解码器永远不来、视频黑屏）。编解码器白名单要按 ogv.js 实际能力写（VP9 SIMD + VP8 基础版）。

## 四、E2E 测试环境（单机全链路）的新增经验

v1 NOTES 的「hbbr loopback 当 CLI 通道」「改配置前先杀进程」等依然适用。v2 这轮新增：

1. **无 systemd 的 VM**：`systemctl` 不可用，被控端要手动起两个进程——`sudo rustdesk --service`（root 服务）+ 桌面会话里的 `rustdesk`（用户进程）。
2. **`rustdesk --server` 缺 `libayatana-appindicator3` 会直接 panic**（tray 库），导致被控端的连接处理进程根本起不来，表现为 relay 配对后秒断（"Failed to receive public key"）。`apt install libayatana-appindicator3-1` 解决。
3. **无 logind 时 `--password` CLI 不可用**（"No --server process found for user main IPC"），永久密码设不了。临时密码**每次重启都重新生成**，只能从桌面 UI 读。E2E 里要么现读现用，要么把密码写进 `RustDesk.toml`。
4. **全 localhost 拓扑的连环坑**：被控端 TCP 连 relay 被 hbbr 当 CLI（v1 NOTES 已记）；配套地，hbbs 要用 `-r <内网IP>:21117` 宣告 relay，否则它给双方发的 relay 地址是 `localhost`，被控端又会回环连 relay 踩回第 1 条。JS 侧和 Go localserver 侧都要把裸 `localhost` 当 IP 类主机（推导 21118/21119），与原生客户端的地址解析一致。
5. **relay 配对有时延**：hbbs 通知被控端到被控端真正加入 hbbr 之间可能隔好几秒。JS 的 `next()` 超时从 12s 提到 18s，对齐原生 `READ_TIMEOUT`（`libs/hbb_common/src/config.rs`），否则慢一点的配对会被误判成超时。
6. **Flutter Web 的 UI 自动化**：CDP 合成鼠标事件（puppeteer `page.mouse.click`）**点不聚焦 Flutter 文本框**（`flt-text-editing-host` 不进编辑态），OS 级事件（xdotool / computerUse）才可以。另外截图像素坐标 ≠ CSS 视口坐标（窗口缩放/DPR），按截图点坐标要先换算。绕过 UI 直接验协议栈的办法：`setByName('session_add_sync', ...)` + `setByName('session_start', ...)` 在 console 里就能驱动整个握手到视频帧（v1 NOTES 的 CDP 方法论同样适用）。
7. **录屏验证**：`RecordScreen` 工具录的可能不是 DISPLAY :1 的活动窗口；`ffmpeg -f x11grab -i :1` 直录最可靠。

## 五、验证清单（每轮改动后）

1. `cd flutter/web/js && npm run build`（codegen + tsc + vite，产物 `dist/index.js`+`vendor.js`）。
2. `cd flutter && flutter build web --release`（或 `deploy/v2/web/build-web-client.sh` 全链路）。
3. 浏览器全新 profile 开页面：控制台应有 `init done`，无 `setByName/getByName` 相关报错。
4. 协议级冒烟：console 里 `setByName('session_add_sync', ...)` + `session_start` → 应依次看到 `Connected to rendezvous server` → `Got relay response` → `Connected to relay server` → `secured` → `input-password` 事件。
5. 完整 UI 链路：输 ID → 连接 → 密码 → 远程画面。

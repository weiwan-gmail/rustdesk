# v2 页面侧独占控制权（control room）

多路 **v2 web** 客户端连同一台被控机时，默认大家都能键鼠（系统级共享输入）。这个可选房间让 **第一个 web 客户端** 独占控制，后来者观看，申请后由当前控制者批准。

**缺省关闭。** 没打开时连接、登录、键鼠、画面与现在完全一样：不注册 `/control`、不强制 view-only、不画条。

不改 hbbs/hbbr、不改被控端 `src/server/`、不改 `deploy/v1`。原生客户端不进房间。

详细开关与协议在本目录；`deploy/v2/README.md` 和 `deploy/v2/web-direct/README.md` 只保留入口说明。

## 打开方式（任一即可）

| 方式 | 作用 |
|---|---|
| `./rustdesk-web-v2 --control` | 服务器模式（默认页面 `:8080`）注册 `/control`，运行时 `config.js` 写 `control: true` |
| `./rustdesk-web-v2-direct --control` | **直连模式（默认 `:8081`）同样支持房间**；`/control` 挂在和页面同一个 `--listen` 上，**不开 8099** |
| `--control-auto-approve` | 隐含 `--control`；全站所有申请立刻批准 |
| `config.js` 里 `control: true` | 只影响页面；服务端没挂 `/control` 时 JS 安静失败，会话不降级 |
| Docker `CONTROL_ROOM=1` | 容器内起 sidecar，Caddy 反代 `/control*` |
| `CONTROL_ROOM_AUTO_APPROVE=1` | 传给 sidecar 的 `--auto-approve` |

`controlBar` 只管条显隐，且 **仅在 `control: true` 时有意义**。`controlBar: false` 时房间仍运行，不画条。

## 真正压在远程画面上的是什么

远程会话里只有底边一条约 **32px 高** 的半透明胶囊（底边居中，不拉满宽）。容器 `pointer-events: none`，只有按钮和 checkbox 能点，其余点击落到远程画面。点「收起」后变成底边一颗约 14px 的圆点（`localStorage` 键 `rd-control-bar=0`）。

实现：`flutter/web/js/src/control_room.ts`。`connection.ts` 只在 `handlePeerInfo` 成功后 `attachControlRoom`，`close()` 时拆掉。view-only 走内存里的 `setViewOnly`，**不写入** peers 的 `localStorage`。

## 调试页不是产品 UI

`controlroom --demo` 在 **8099** 上提供三列 A/B/C 调试页：每列都有 Connect/Request/Approve 按钮、JSON 状态、再加一条模拟胶囊。那是用来验证房间协议的，**会占满浏览器**，不会出现在远程桌面里。

以前 walkthrough 截图里「控制相关占满屏」，拍的就是这个调试页，不是 32px 的 overlay。

```bash
cd deploy/v2/controlroom
go run ./cmd/controlroom --listen :8099 --demo
# 打开 http://127.0.0.1:8099/
```

建议走查：

1. A Connect → `you: controller`
2. B Connect → `you: viewer`，A 的 `viewerCount` 为 1
3. B Request → B 仍是 viewer，`youRequested: true`；A 出现 `pendingIp`
4. A 点 Approve（或勾选「Auto-approve next」，勾选变化会 `setAutoApprove`，若已有 pending 会立刻过）
5. B 变为 controller，A 变为 viewer
6. B 勾选 Auto-approve next，C Connect + Request → C 立刻成为 controller

## 端口：8099 和其他口不是一类

8099 **不是** RustDesk 协议口，也不是给人打开页面的口。它只给 **Docker sidecar**（以及单独跑 `cmd/controlroom`）用。浏览器始终连 **页面同源** 的 `/control`。

| 端口 | 谁在听 | 干什么 | 浏览器怎么碰到 |
|---|---|---|---|
| **8080**（compose 映射到容器 80） | Caddy / `rustdesk-web-v2` | 页面 + `/ws/id`、`/ws/relay`；`--control` 时还有 `/control` | 打开网页 |
| **8081** | `rustdesk-web-v2-direct` | 页面 + `/direct`；`--control` 时还有 `/control` | 直连版网页 |
| **21115 / 21116** | hbbs | NAT 探测、ID 注册 / 心跳 | 原生客户端 |
| **21117** | hbbr | 原生中继 | 原生客户端 |
| **21118 / 21119** | hbbs/hbbr 的 WebSocket，或被控直连口 | 网页信令/中继，或直连 TCP | 经 Caddy `/ws/*`，或直连被控 **21118** |
| **8099** | `rustdesk-web-controlroom` sidecar | 只处理房间 JSON | **不对外发布**。Compose 绑 `127.0.0.1:8099`，Caddy 把 `/control*` 转到它 |

单二进制 `--control` **不会**再开 8099：`/control` 和页面同端口。8099 只出现在：

- `CONTROL_ROOM=1` 的 Docker 入口脚本（loopback）
- 本目录 `go run ./cmd/controlroom --listen :8099`

`2111x` 传画面/键鼠/信令。房间 **不传画面**，只协调谁能点键盘鼠标。

## 直连模式（8081）

可以，而且已经接好。与服务器模式同一套 `controlroom` 包。

```bash
./rustdesk-web-v2-direct --listen :8081 --control
./rustdesk-web-v2-direct --listen :8081 --control --control-auto-approve
```

房间按被控 **IP** 分：`192.168.1.50` 与 `192.168.1.50:21118` 同一间（`NormalizeTarget`）。RustDesk ID 原样（大小写不敏感）。

## 自动批准：按客户端，不是按房间

条上「下次自动批准」是 **每个 web 成员（一次 WS 连接 / 一个浏览器）** 自己的开关，不是按被控端、也不是按房间全局。

申请会不会立刻过，只看 **当前控制者** 的 `autoApprove`（或进程级全局开关）：

- 勾选：写入该浏览器 `localStorage`（`rd-control-auto-approve`），join 时带 `?autoApprove=1`，勾选变化发 `setAutoApprove`
- A 勾了并把控制权交给 B 之后，B **不继承** A 的勾选；后续申请要等 B 自己勾，或 B 手动批准
- A 再当回控制者时，只要这个浏览器还记着，会再自动过
- `--control-auto-approve` / `CONTROL_ROOM_AUTO_APPROVE=1` 是进程级 `globalAutoApprove`，所有房间立刻生效，不再看谁勾了框

无控制者（断开或点「释放」）时，第一条申请直接获得（空位，没人可点批准）。

## 调试与回归

```bash
# 房间规则 + 两路真实 WS
cd deploy/v2/controlroom && go test ./...

# 开关关闭时没有 /control
cd deploy/v2/web/localserver && go test ./...
cd deploy/v2/web-direct/server && go test ./...

# 页面开关 / 文案
cd flutter/web/js && npm test
```

功能关闭冒烟：二进制不加 `--control` 时 `GET /control` 应为 404。打开后非 WebSocket 的 `GET /control?target=desk` 应为 400（需要升级）。

Docker：`CONTROL_ROOM` 未设时 `/srv/config.js` 不含 `control: true`，Caddy 的 `control.handle` 为空注释，sidecar 不起。

## 明确不做

- 不改 hbbs/hbbr、不改被控端输入锁
- 不把原生客户端纳入房间
- 缺省不打开房间（避免改变现网行为）
- 有控制者时不把控制权偷偷交给申请者，除非批准或自动批准

# rustdesk-web-v2-direct（v2 直连版 Web 客户端）

浏览器里**按 IP 直连**被控 RustDesk 客户端，**无需运行 hbbs/hbbr 服务器**。类似 noVNC 的 websockify 模式：一个自包含二进制，既发页面又把浏览器的 WebSocket 桥接到被控端的直连 TCP 端口。

```
浏览器 ──ws/wss──► rustdesk-web-v2-direct ──裸 TCP──► 被控:21118(直连端口)
        :8081         (页面 + WS→TCP 桥)             (开了「直接IP访问」)
```

与 `deploy/v2/web`（服务器模式）完全独立：不同 exe、不同目录，可同机不同端口并存。两者共用仓库根的 `flutter/` 树，直连由本目录 `config.js` 的 `direct: true` 打开。

本目录的 Go 服务器（`server/`）是与客户端版本无关的交付基础设施，与 v1 直连版（`deploy/v1/web-direct/server`）同源；只有内嵌的静态客户端不同（这里是当前 `flutter/` 树的构建）。

## 与服务器模式的区别

| | rustdesk-web-v2（服务器模式） | rustdesk-web-v2-direct（直连模式） |
|---|---|---|
| 寻址 | 按 ID | 按 IP |
| 需要 hbbs/hbbr | 需要 | **不需要** |
| 网络 | 可跨 NAT/公网（走中继） | 仅局域网/IP 可达 |
| 被控端配置 | 填服务器 + Key | 开「直接 IP 访问」 |
| 加密 | 中继加密握手 | 明文（与原生直连一致） |

## 使用

```bash
cd deploy/v2/web-direct
./build.sh                 # 构建 v2 web 客户端并编译当前平台二进制
./build.sh --all           # 交叉编译 linux/windows/macOS 单文件

./rustdesk-web-v2-direct                          # http://localhost:8081
./rustdesk-web-v2-direct --listen :9000 --open    # 指定端口并自动开浏览器
```

打开页面后，在「Remote ID」输入框里**直接填被控端的 IP**（如 `192.168.1.50` 或 `192.168.1.50:21118`），输入被控端密码即可连接。客户端识别到输入是 IP 就自动走直连（对齐原生客户端 `is_ip_str` 的行为），填 ID 则仍尝试走服务器。

## 被控端设置

被控端就是普通 RustDesk 客户端，开启「直接 IP 访问」：

- GUI：设置 → 网络 →「直接 IP 访问 / Direct IP access」打开（默认端口 21118，可用 `direct-access-port` 改）。
- 或配置文件 `RustDesk2.toml` 的 `[options]` 加 `direct-server = 'Y'`。
- 确保该端口对运行 `rustdesk-web-v2-direct` 的机器网络可达。

## 参数

- `--listen addr`：页面监听地址，默认 `:8081`。
- `--direct-port n`：允许的目标端口，默认 `21118`（被控端直连端口）。
- `--allow-cidr a,b,c`：允许直连的目标网段（默认仅回环/私网/链路本地）。
- `--allow-any`：关闭目标 IP 限制（**危险**：代理会变成开放 TCP 中继/SSRF，勿对公网开放）。
- `--tls-cert/--tls-key`：可选 TLS；不提供即纯 HTTP（内网/本机免证书）。
- `--base-path`、`--open`。
- `--control`：打开页面侧独占控制权（`/control`）。默认关闭，行为与现在一致。
- `--control-auto-approve`：所有申请立刻批准（隐含 `--control`）。

勾选条上「下次自动批准」只对该浏览器当控制者期间有效；命令行自动批准是全站的。

## 安全说明

`/direct` 是一个 WS→TCP 桥。默认只允许**回环/私网/链路本地**目标且只允许 `21118` 端口，防止它沦为开放代理。不要用 `--allow-any` 把它暴露到不可信网络。

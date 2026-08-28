# 内网根 CA 配置指南（让 Web 客户端走 HTTPS / 安全上下文）

> 适用 v1 与 v2 的 Web 客户端部署。本文与代码无关，纯部署/运维向。

## 为什么需要它

Web 客户端的某些浏览器能力（**WebCodecs 硬解**、`SharedArrayBuffer` 多线程解码、PWA、剪贴板 `navigator.clipboard`）只在**安全上下文**（HTTPS / localhost / `file://`）可用。纯 HTTP 内网 IP（如 `http://192.168.1.10:8080`）不是安全上下文，这些能力会被浏览器禁用。

**不强制**：不配证书的话，HTTP 内网部署照常能用（视频回退软件解码，只是慢）。配了证书走 HTTPS 后，这些能力自动解锁。这是**可选优化**。

## 核心思路：一个内部根 CA，客户端只装一次

**不要**把同一个自签服务器证书拷到多台机器（SAN 不匹配 + 私钥扩散）。正确做法是建**一个内部根 CA**：

```
内部根 CA (rootCA.pem + rootCA-key.pem)   ← 只建一次，私钥保密
   ├─ 签给 机器A 的服务器证书 (SAN: 192.168.1.10)
   ├─ 签给 机器B 的服务器证书 (SAN: 192.168.1.11)
   └─ 签给 机器C 的服务器证书 (SAN: rustdesk.internal)

客户端：只需把 rootCA.pem（公钥）装进信任库【一次】
        → 之后所有用该 CA 签的部署都被信任，随便加机器客户端都不用再动
```

- 服务器证书按 **SAN（IP/主机名）** 绑定——给 A 签的证访问不了 B，所以**每台服务器各签各的**。
- 根 CA 私钥（`rootCA-key.pem`）**保密**；只把 `rootCA.pem`（公钥）发给客户端。

## 方案一：mkcert（离线 CA，推荐小规模）

mkcert 的"CA"不是常驻进程，就是磁盘上的根密钥对，离线签名。几台~几十台机器用这个最省事。

### 1. 建 CA（只做一次，在一台信得过的机器上）

```bash
mkcert -install          # 生成根 CA 并装进本机信任库
mkcert -CAROOT           # 查看 CA 目录，通常是 ~/.local/share/mkcert
                         # 里面: rootCA.pem(公钥,发客户端) + rootCA-key.pem(私钥,保密)
```

### 2. 签发服务器证书

- **集中签发（推荐）**：就在 CA 机器上签所有证，签完把"叶子证书+私钥"拷到对应服务器（CA 私钥不出门）。
- **分发签发**：把 `rootCA.pem`+`rootCA-key.pem` 拷到签发机的 `$(mkcert -CAROOT)`（或设 `CAROOT=/path`），即可用同一 CA 发证。

```bash
mkcert 192.168.1.10                    # 机器A
mkcert 192.168.1.11 172.30.0.5         # 机器B（可多个 SAN）
mkcert rustdesk.internal localhost     # 主机名也行
# 产出 <名>.pem(证书) 和 <名>-key.pem(私钥)，拷到对应服务器
```

### 3. 挂到 Web 客户端部署

- **localserver 单二进制**：
  ```bash
  ./rustdesk-web-v2 --server 192.168.1.10 \
      --tls-cert 192.168.1.10.pem --tls-key 192.168.1.10-key.pem
  ```
  （v1 用 `rustdesk-web`，参数相同）
- **Caddy（compose/镜像）**：把自动 HTTPS 换成指定证书，Caddyfile 站点块里加：
  ```
  tls /etc/certs/server.crt /etc/certs/server.key
  ```

### 4. 客户端安装根证书（唯一一次）

把 `rootCA.pem` 分发到每个客户端：

- **Windows**：双击 → 安装证书 → 本地计算机 → 受信任的根证书颁发机构。
- **macOS**：钥匙串访问 → 系统 → 导入 → 双击设为"始终信任"。
- **Linux / Chrome**：
  ```bash
  certutil -d sql:$HOME/.pki/nssdb -A -t "C,," -n MyInternalCA -i rootCA.pem
  ```
- **Firefox**：用自己独立的信任库——设置 → 隐私与安全 → 证书 → 导入；或企业策略推送。

## 方案二：step-ca（在线 CA 服务器，机器多/要自动续期时用）

[smallstep](https://smallstep.com) 的 `step-ca` 是常驻 CA 守护进程，支持 **ACME**（与 Let's Encrypt 同协议），服务器可自动申请/续期。

### 1. 初始化 + 启动

```bash
step ca init --name="MyInternalCA" --dns="ca.internal" --address=":443" --provisioner="admin"
# 生成在 $(step path)（默认 ~/.step/）:
#   certs/root_ca.crt(根证书,发客户端)  secrets/...(私钥,保密)  config/ca.json

step-ca $(step path)/config/ca.json
# ACME directory: https://ca.internal/acme/acme/directory
```

### 2. 服务器自动领证（ACME）

```bash
# 手动领
step ca certificate 192.168.1.10 server.crt server.key --ca-url https://ca.internal --root root_ca.crt
```

或让 **Caddy 直接对接 step-ca 自动签发/续期**（Caddyfile 全局配置）：

```
{
  acme_ca https://ca.internal/acme/acme/directory
}
```

### 3. 客户端装根证书

```bash
step ca root root_ca.crt --ca-url https://ca.internal   # 从 CA 拉根证书
# 再按平台装进信任库（同方案一第 4 步）
```

## 两方案对比

| | mkcert（离线 CA） | step-ca（在线 CA） |
|---|---|---|
| 常驻进程 | 无 | 有（CA + ACME） |
| 发证 | 手动签、手动拷 | 服务器自动申请/续期 |
| 适用 | 几台~几十台，偶尔发证 | 机器多、要自动化 |
| 配置复杂度 | 极低 | 中 |
| Caddy 集成 | 手动 `tls` 挂证 | ACME 自动签 |

**建议**：几台部署机器 + 客户端装一次根 → mkcert 足够；机器多到手动发证嫌烦 → 上 step-ca。

## 注意事项

1. **根 CA 私钥保密**：泄露后别人能签出你所有客户端都信任的假证书。只发 `rootCA.pem`（公钥）给客户端。
2. **Firefox 独立信任库**：Chrome/Edge 用系统库，Firefox 要单独装或企业策略推。
3. **有效期**：服务器证书建议 ≤1 年（浏览器对服务端证书有效期有限制），根 CA 可签 10 年+。
4. **完全可选**：不配证书时 HTTP 内网部署仍可用（软解回退），只是没有硬解/多线程加速。

## 其他零证书替代（临时/个人）

- **localhost 访问**：本机单二进制 + 浏览器开 `http://localhost:8080`，天然安全上下文，无需证书。
- **SSH 端口转发**：`ssh -L 8080:localhost:8080 user@server`，浏览器开 `http://localhost:8080`，零证书。
- **Chrome 测试参数**（仅调试，勿用于生产）：`--unsafely-treat-insecure-origin-as-secure=http://192.168.1.10:8080`。

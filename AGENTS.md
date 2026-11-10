# MCP 安装与使用手册（给 AI）

> **这份文档的目的：让 AI 自己把 MCP-DM 装起来，然后使用它。**
> 人类的项目介绍看 [README.md](README.md)，代码改动规范看 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 一、这是什么

MCP-DM 是一个自托管的知识库服务器，提供 **MCP 工具**给 AI 使用。

你（AI）可以通过它：

- 检索过去记录的事实、决策、事故、配置
- 把新发现写进去，供以后的自己使用

**核心价值**：跨会话的长期记忆。用户下次说「上次那个端口」时，你不用猜。

---

## 二、安装（二选一）

### 方式 A：用现成二进制

从 [Releases](https://github.com/aizexintong/MCP-DM/releases) 下载对应平台：

| 平台 | 文件 |
|---|---|
| Linux x86_64（静态，推荐） | `MCP-DM-linux-amd64` |
| Windows x86_64 | `MCP-DM-windows-amd64.exe` |

```bash
# Linux
chmod +x MCP-DM-linux-amd64
./MCP-DM-linux-amd64
```

```powershell
# Windows
.\MCP-DM-windows-amd64.exe
```

首次启动自动初始化数据库到 `./data/`，**无需预装 PostgreSQL**。

### 方式 B：从源码编译

```bash
git clone https://github.com/aizexintong/MCP-DM.git
cd MCP-DM
cargo build --release
./target/release/MCP-DM
```

> macOS 用户：`postgresql_embedded` 缺 macOS 预编译包，需要本地已装 PostgreSQL
> 或自行配置。不建议在 macOS 上首次尝试。

### 启动后确认

```
Web  http://0.0.0.0:8767
MCP  http://0.0.0.0:8765/mcp
```

用浏览器打开 `http://127.0.0.1:8767` 能看到界面 = 启动成功。

### 端口与目录

| 环境变量 | 默认 | 说明 |
|---|---|---|
| `MCP_PORT` | `8765` | MCP 端点端口 |
| `WEB_PORT` | `8767` | 网页端口 |
| `DATA_DIR` | `./data` | 数据目录（含 PG 数据） |
| `PG_PORT` | `55432` | 内嵌 PG 端口（仅 Unix socket） |

迁移到新机器：**把 `data/` 整个打包带走**即可，不用重装数据库。

---

## 三、接入 MCP 客户端

MCP 端点是 **streamable-http** 协议：

```
http://<服务器IP>:8765/mcp
```

### 3.1 Claude Desktop

编辑配置（macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`，
Windows: `%APPDATA%\Claude\claude_desktop_config.json`）：

```json
{
  "mcpServers": {
    "memory": {
      "url": "http://127.0.0.1:8765/mcp"
    }
  }
}
```

### 3.2 Claude Code / Cursor / Cline 等

命令行添加：

```bash
# Claude Code
claude mcp add --transport http memory http://127.0.0.1:8765/mcp

# Cursor: 在 ~/.cursor/mcp.json
```

```json
{
  "mcpServers": {
    "memory": {
      "url": "http://127.0.0.1:8765/mcp"
    }
  }
}
```

### 3.3 DSH（DeepSeek Harness）

在 profile 的 `cordis.patch.yml` 加：

```yaml
- id: mcp-memory
  name: '@deepseek-ai/dsh-mcp-client'
  config:
    serverName: memory
    transport: streamable-http
    url: http://127.0.0.1:8765/mcp
    toolCallTimeoutMs: 30000
    failOnStartupError: false
    reconnect:
      enabled: true
      initialDelayMs: 1000
      maxAttempts: 20
```

### 3.4 验证连接

配置后，客户端工具列表里应该出现 8 个以 `mem_` 开头的工具：

```
mem_search  mem_get  mem_add  mem_mark
mem_like    mem_categories  mem_stats  mem_timeline
```

连不上时手动测：

```bash
curl -X POST http://127.0.0.1:8765/mcp \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize",
       "params":{"protocolVersion":"2025-06-18","capabilities":{},
                 "clientInfo":{"name":"test","version":"1"}}}'
```

返回 `200` + `mcp-session-id` 响应头 = 正常。

---

## 四、工具手册

### 4.1 `mem_search` — 检索（**最常用**）

```json
{
  "query": "端口",
  "limit": 20,
  "sort": "relevance",
  "cat": "network.pve",
  "days": 30,
  "state": "current"
}
```

| 参数 | 必填 | 说明 |
|---|---|---|
| `query` | 否* | 关键词。中文按标点切词，效果一般，靠模糊匹配兜底 |
| `cat` | 否* | 分类前缀，`mc-server` 会包含所有子分类 |
| `sort` | 否 | `relevance`（默认）/ `newest` / `marked` |
| `limit` | 否 | 默认 20 |
| `days` | 否 | 只要最近 N 天 |
| `state` | 否 | `current` / `archived` / `obsolete` |

\* `query` 和 `cat` 至少给一个；只给 `cat` = 列出该分类全部条目。

**返回**：

```json
{
  "status": "found",
  "query": "端口",
  "total": 8,
  "results": [
    {
      "id": 56,
      "title": "iStoreOS 软路由：出口、防火墙与代理",
      "cat_path": "network.pve.routing-dns",
      "snippet": "系统\n  iStoreOS 24.10.8，内核 6.6.144…",
      "cause": "软路由配置复杂，出口策略、代理规则、端口转发都没有记录",
      "effect": "明确了所有出口路径与监听端口，确认内网服务对外只走 FRP",
      "state": "current",
      "mark_level": 3,
      "score": 0.2538,
      "updated_at": "2026-11-10T18:41:00Z"
    }
  ]
}
```

**`status` 只有两个值**：
- `"found"` — 查到了，`total` 可能为 0（`results: []`）
- `"no_seed"` — 知识库确实没有相关内容

### 4.2 `mem_get` — 读全文

```json
{ "id": 56 }
```

返回完整 `body`（Markdown）、`links_out` / `links_in`（引用关系）、`state_reason`。

`mem_search` 的 `snippet` 只有 200 字符，需要细节时用它。

### 4.3 `mem_add` — 写入

```json
{
  "title": "FRP 端口映射调整",
  "body": "把 25565 从 62132 改到 62133…",
  "cause": "旧端口被扫描爆破",
  "effect": "已切换，客户端需更新",
  "cat_path": "network.pve.ports",
  "tags": ["FRP", "端口"],
  "source": []
}
```

| 参数 | 必填 | 说明 |
|---|---|---|
| `title` | **是** | 一句话说清是什么 |
| `cat_path` | **是** | 点号分层，如 `mc-server.changelog` |
| `body` | 否 | Markdown 正文 |
| `cause` | 否 | **为什么**会发生 / 为什么需要它 |
| `effect` | 否 | 导致了什么 / 最终结论 |
| `tags` | 否 | 字符串数组 |
| `source` | 否 | 来源文件路径 |

**写入前先 `mem_search`**，避免重复记录同一件事。

### 4.4 `mem_categories` — 看分类树

无参数。返回所有分类及条目数：

```json
{
  "categories": [
    { "path": "mc-server", "label": "MC服务器", "depth": 1, "count": 40 },
    { "path": "mc-server.changelog", "label": "变更历史", "depth": 2, "count": 14 }
  ]
}
```

**`path` 是英文标识，`label` 是中文显示名**。新建条目时用 `path`。

### 4.5 `mem_timeline` — 时间线

```json
{ "cat": "mc-server", "limit": 100 }
```

按 `updated_at` 倒序。用来理解一件事的演进过程。

### 4.6 `mem_stats` — 规模

```json
{ "entries": 61, "categories": 24, "links": 43, "marked": 1 }
```

### 4.7 `mem_mark` — 收藏 / 精选

```json
{ "id": 56, "level": 3 }
```

| level | 含义 |
|---|---|
| 0 | 取消 |
| 1 | 收藏 |
| 2 | 精选 |
| 3 | 收藏 + 精选 |

### 4.8 `mem_like` — 点赞

```json
{ "id": 56, "delta": 1 }
```

---

## 五、调用策略

### 必查场景

| 用户说的话 | 该做什么 |
|---|---|
| 「上次那个…」「之前说的…」「那个配置」 | 先 `mem_search` 相关关键词 |
| 「端口是多少」「种子是什么」「md5 多少」 | 先 `mem_search`，别凭印象答 |
| 「我们之前怎么处理的」 | `mem_search` + `mem_timeline` |
| 开始一个新任务 | 先 `mem_search` 看有没有历史方案 |

### 必写场景

| 情况 | 该做什么 |
|---|---|
| 完成了配置变更 | `mem_add`，写清 `cause` / `effect` |
| 排查出根因 | `mem_add`，把现象和原因都记下 |
| 做了技术选型 | `mem_add`，记录为什么选它、为什么不选别的 |
| 用户说「记一下」 | `mem_add` |

### 三条铁律

1. **开场必查** — 不查就回答等于重复劳动
2. **`no_seed` 就问人** — 知识库没有就是没有，不要编
3. **先看 `state`** — `archived` 只说明当时状态，`obsolete` 不要引用

### 分类选择

```
mc-server/          MC 服务器
├─ changelog        变更历史
├─ deployment       部署配置
├─ mods             模组
├─ gameplay         玩法
├─ incidents        事故
└─ testing          测试验证

network/pve/        网络与虚拟化
├─ machines         机器清单
├─ ports            端口分配
└─ routing-dns      路由与 DNS

dev/                开发项目
kb/                 知识库自身
misc-root/          杂项
```

完整列表调用 `mem_categories`。

---

## 六、写高质量的条目

好的条目长这样：

```
title:  外部主机 VM 801 的 FRP 端口映射
cause:  MC 服务需要对外，但内网无公网 IP
effect: 通过 FRP 把 25565 映射到 62132，客户端连 120.76.139.101:62132
body:
  FRP 服务端：120.76.139.101:1210
  配置文件：/etc/frp/frpc-aurelia.toml

  端口映射
  SSH        22     → 16148
  Minecraft  25565  → 62132
  Bedrock    19132  → 64509

  注意：25575 RCON 不要暴露到公网
```

**要点**：

- `title` 说清楚「是什么」，不要「关于 xxx 的记录」
- `cause` 和 `effect` **必须写**，这是它的价值所在
- `body` 用 Markdown，表格、列表、代码块都支持
- 具体数字、路径、IP 都写上，不要「那个文件」「某个端口」

---

## 七、故障排查

| 症状 | 原因 | 解决 |
|---|---|---|
| 工具列表里没有 `mem_*` | 客户端没连上 | 检查 URL、端口、客户端配置是否重启 |
| `initialize` 返回 404 | URL 少了 `/mcp` | 完整地址是 `http://host:8765/mcp` |
| 连接超时 | 服务未启动 / 防火墙 | `curl http://127.0.0.1:8765/mcp` 测 |
| `no_seed` 但应该有内容 | 关键词不匹配 | 换更短、更通用的词；或先 `mem_categories` 找分类再按 `cat` 查 |
| 搜索结果排序奇怪 | 中文分词弱 | 用 `sort: "newest"` 或加 `cat` 缩小范围 |
| 服务起不来（Linux） | 端口占用 | `ss -lntp \| grep -E '8765\|8767'` |
| `initdb: cannot be run as root` | PG 不允许 root | 用普通用户跑 |
| 服务卡在启动 | 残留 postgres 进程 | `ps aux \| grep postgres` 后 kill |

查看日志：

```bash
# systemd 部署
journalctl -u MCP-DM -n 50 -f

# 直接运行
RUST_LOG=debug ./MCP-DM
```

---

## 八、curl 手动调用示例

不想装客户端时，可以直接发 JSON-RPC：

```bash
URL=http://127.0.0.1:8765/mcp

# 1. 初始化，拿 session id
SID=$(curl -s -D- -o /dev/null -X POST $URL \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"cli","version":"1"}}}' \
  | grep -i 'mcp-session-id' | cut -d' ' -f2 | tr -d '\r')

# 2. 调用工具
curl -s -X POST $URL \
  -H "Content-Type: application/json" \
  -H "Accept: application/json, text/event-stream" \
  -H "mcp-session-id: $SID" \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call",
       "params":{"name":"mem_search","arguments":{"query":"端口","limit":5}}}'
```

---

## 九、部署为常驻服务

### Linux (systemd)

```ini
# /etc/systemd/system/mcp-dm.service
[Unit]
Description=MCP-DM Knowledge Base
After=network.target

[Service]
Type=simple
User=YOUR_USER
WorkingDirectory=/opt/mcp-dm
Environment=DATA_DIR=/opt/mcp-dm/data
Environment=MCP_PORT=8765
Environment=WEB_PORT=8767
ExecStart=/opt/mcp-dm/MCP-DM
Restart=always
RestartSec=5
KillMode=mixed
TimeoutStopSec=30

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now MCP-DM
sudo systemctl status MCP-DM
```

> `User=` 不能是 root，PostgreSQL 拒绝以 root 运行。

### Windows

用 [NSSM](https://nssm.cc/) 或任务计划程序注册为服务：

```powershell
nssm install MCP-DM "C:\mcp-dm\MCP-DM.exe"
nssm set MCP-DM AppEnvironmentExtra DATA_DIR=C:\mcp-dm\data
nssm start MCP-DM
```

---

## 十、自检清单

装完后逐条确认：

- [ ] `curl http://127.0.0.1:8767/api/stats` 返回 JSON
- [ ] MCP 客户端工具列表有 8 个 `mem_*`
- [ ] `mem_stats` 能返回条目数
- [ ] `mem_search` 传一个已知关键词能查到
- [ ] `mem_add` 能成功写入，再 `mem_search` 能查到
- [ ] 重启服务后数据还在（验证持久化）
- [ ] `data/` 目录已加入备份计划

全部通过 = 装好了。

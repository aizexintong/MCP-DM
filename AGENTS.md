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

**最简单的方式**：只在 profile 顶层挂 MCP 客户端，不需要写预设。

`~/.dsh/profiles/web/cordis.patch.yml`：

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

**进阶：做成 Agent 预设**，把 MCP 和「开场必查」的规则绑在一起，新会话自动生效。
下面这份是可直接用的完整配置，语法坑都在注释里标了：

```yaml
- insert:
    - id: preset-memory
      name: '@deepseek-ai/dsh-agent-preset'
      config:
        id: memory-first              # 会话里保存的预设标识
        name: 记忆库优先               # 界面上显示的名字
        description: 每次开场先查知识库
        order: 0                      # 排在其他预设前面
        plugins:
          - id: persona
            name: '@deepseek-ai/dsh-persona'
            config:
              prefix: You are a coding agent powered by the {{model}} model.
              suffix: |
                ## 知识库使用规则（强制）
                开场必做：任何新任务前先调 mcp__memory__mem_search。
                1. 开场必查，不查等于重复劳动
                2. status: no_seed 就问人，不要编
                3. 先看 state：current 采信，archived 只说明当时状态，obsolete 不要引用
          - id: agent-instructions
            name: '@deepseek-ai/dsh-agent-instructions'
            config:
              maxBytes: 65536
          - id: mcp-memory-server
            name: '@deepseek-ai/dsh-mcp-client'
            config:
              serverName: memory
              transport: streamable-http
              url: http://127.0.0.1:8765/mcp
              failOnStartupError: false
          - id: tool-bash
            name: '@deepseek-ai/dsh-tool-bash'
          - id: tool-fs
            name: '@deepseek-ai/dsh-tool-fs'
          - id: tool-fs-search
            name: '@deepseek-ai/dsh-tool-fs-search'
          - id: tool-jobs
            name: '@deepseek-ai/dsh-tool-jobs'
          - id: skill-filesystem
            name: '@deepseek-ai/dsh-skill-filesystem'
          - id: tool-skill
            name: '@deepseek-ai/dsh-tool-skill'

          # 分组插件必须这么写，三处细节缺一不可
          - id: planning
            name: cordis:group           # 固定字符串，不是包名
            group: true                   # 必须
            isolate:
              planMode: true              # 声明要隔离的服务
            config:                       # 子插件放这里，是数组
              - id: plan-mode
                name: '@deepseek-ai/dsh-plan-mode'

          - id: compaction
            name: cordis:group
            group: true
            isolate:
              compaction: true
              toolResultPruner: true
            config:
              - id: compaction-basic
                name: '@deepseek-ai/dsh-compaction-basic'
              - id: command-compact
                name: '@deepseek-ai/dsh-command-compact'
              - id: tool-result-pruner
                name: '@deepseek-ai/dsh-compaction-tool-result-pruner'
                config:
                  thresholdChars: 8192

          - id: delegation
            name: cordis:group
            group: true
            isolate:
              workflowEngine: true
            config:
              - id: tool-subagent-control
                name: '@deepseek-ai/dsh-tool-subagent-control'
              - id: tool-subagent-list-agents
                name: '@deepseek-ai/dsh-tool-subagent-control/list-agents'
              - id: tool-subagent
                name: '@deepseek-ai/dsh-tool-subagent'
                config:
                  provider: spawn
                  toolName: subagent
                  backgroundMode: continuable
              - id: tool-subagent-fork
                name: '@deepseek-ai/dsh-tool-subagent'
                config:
                  provider: fork
                  toolName: subagent_fork
                  backgroundMode: continuable

          - id: tool-ask-user
            name: '@deepseek-ai/dsh-tool-ask-user'
          - id: tool-todo
            name: '@deepseek-ai/dsh-tool-todo'
            config:
              allowParallelInProgress: true   # 必填，漏了会启动失败
          - id: tool-web
            name: '@deepseek-ai/dsh-tool-web'
          - id: present
            name: '@deepseek-ai/dsh-tool-present'
```

#### 预设加载失败的三种典型报错

| 报错 | 原因 | 修法 |
|---|---|---|
| `X (cordis:group): Cannot read properties of undefined (reading 'map')` | 分组插件写成了 `plugins:` 或用了不存在的包名 | 用 `name: cordis:group` + `group: true` + `config:` 数组 |
| `tool-todo: invalid config: $.allowParallelInProgress missing required value` | 缺必填项 | 加 `allowParallelInProgress: true` |
| `Cannot find module '@deepseek-ai/dsh-xxx'` | 包名写错 | 对照 `node_modules/@deepseek-ai/dsh-web-app/presets/standard.patch.yml` |

**最可靠的办法**：打开官方标准预设逐项对照

```bash
cat node_modules/@deepseek-ai/dsh-web-app/presets/standard.patch.yml
```

#### 另一个会挡住预设加载的问题

如果启动时看到：

```
dsh: warning: 1 entry did not activate
image-offload (@deepseek-ai/dsh-compaction-image-offload): Error:
session message projection "image/offload" is already registered
```

说明有别的插件抢先注册了 `image/offload` 投影。查：

```bash
grep -rn "image/offload" ~/.dsh/profiles/*/node_modules/*/index.js
```

找到那个重复注册的插件，把它的 `registerMessageProjection` 调用删掉——投影由核心提供，
它只负责 append 事件就够了。`try/catch` 不管用，因为它先注册、核心后注册时炸的是核心。

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

---

## 十一、搬家清单（换机器 / 迁移部署）

数据能直接搬，但有 **4 处配置写死了绝对路径或 IP**，必须一起改，漏一个就起不来。

### 1. 打包数据

```bash
systemctl stop mcp-dm
tar -czf mcp-dm-data.tar.gz -C /data/projects/MCP-DM data
```

`data/` 里就是全部数据（含 PostgreSQL 集群），不需要 `pg_dump`。

### 2. 新机器解开

```bash
mkdir -p /opt/mcp-dm
tar -xzf mcp-dm-data.tar.gz -C /opt/mcp-dm     # 得到 /opt/mcp-dm/data
cp MCP-DM-linux-amd64 /opt/mcp-dm/MCP-DM
chmod +x /opt/mcp-dm/MCP-DM
chown -R YOUR_USER /opt/mcp-dm
```

> **不能用 root 跑**。PostgreSQL 直接拒绝：`initdb: cannot be run as root`。

### 3. 改 PG 的 socket 路径（最容易漏）

`data/pg/postgresql.conf` 里这一行是**绝对路径**：

```conf
unix_socket_directories = '/data/projects/MCP-DM/data/pg/run'
```

换成新路径：

```bash
sed -i 's|/data/projects/MCP-DM|/opt/mcp-dm|g' /opt/mcp-dm/data/pg/postgresql.conf
```

漏了这行的症状：服务卡在「启动 PostgreSQL」，或报 socket 不存在。

### 4. 环境变量

| 变量 | 典型值 | 说明 |
|---|---|---|
| `BIND` | `10.0.0.0/8` | **默认只有 127.0.0.1**，局域网要显式放开 |
| `DATA_DIR` | `/opt/mcp-dm/data` | 改了要同步第 3 步 |
| `WEB_PORT` | `8767` | 网页端口 |
| `MCP_PORT` | `8765` | MCP 端口 |
| `PG_PORT` | `55432` | 内嵌 PG（仅 socket） |

### 5. systemd 单元

```ini
# /etc/systemd/system/mcp-dm.service
[Unit]
Description=MCP-DM 文档记忆系统
After=network.target

[Service]
Type=simple
User=YOUR_USER
Group=YOUR_GROUP
WorkingDirectory=/opt/mcp-dm

Environment=BIND=10.0.0.0/8
Environment=DATA_DIR=/opt/mcp-dm/data
Environment=WEB_PORT=8767
Environment=MCP_PORT=8765
Environment=PG_PORT=55432
Environment=RUST_LOG=info,actix_web=warn

ExecStart=/opt/mcp-dm/MCP-DM

Restart=always
RestartSec=5
KillMode=mixed
TimeoutStopSec=30
StandardOutput=append:/opt/mcp-dm/logs/mcp-dm.log
StandardError=append:/opt/mcp-dm/logs/mcp-dm.log

NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```

```bash
mkdir -p /opt/mcp-dm/logs && chown -R YOUR_USER /opt/mcp-dm/logs
systemctl daemon-reload && systemctl enable --now mcp-dm
```

### 6. 客户端要改的地址

| 客户端 | 改什么 |
|---|---|
| Claude Desktop / Cursor | MCP 配置里的 `url` |
| Claude Code | `claude mcp remove memory` 再 `add` |
| DSH 顶层 `mcp-memory` | `cordis.patch.yml` 的 `url` |
| DSH 预设 `mcp-memory-server` | 预设里的 `url`（**两处都要改**） |

### 7. 迁移后验证

```bash
systemctl is-active mcp-dm
grep -E "MCP-DM v|监听地址" /opt/mcp-dm/logs/mcp-dm.log | tail -2

curl -s http://127.0.0.1:8767/api/stats
# 期望 {"cats":26,"entries":75,"links":43,"marked":2}

curl -s -o /dev/null -w "%{http_code}\n" http://<新IP>:8767/api/stats
# 期望 200

curl -s -o /dev/null -w "%{http_code}\n" -X POST http://127.0.0.1:8765/mcp \
  -H 'Content-Type: application/json' \
  -H 'Accept: application/json, text/event-stream' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize",
       "params":{"protocolVersion":"2025-06-18","capabilities":{},
                 "clientInfo":{"name":"t","version":"1"}}}'
# 期望 200
```

### 搬家最容易踩的四个坑

| 坑 | 症状 | 原因 |
|---|---|---|
| 用 root 启动 | 卡在「初始化 PostgreSQL」 | `initdb` 拒绝 root |
| 没改 `unix_socket_directories` | 卡在「启动 PostgreSQL」 | PG 配置写死绝对路径 |
| 没设 `BIND` | 本机能开，别的机器 403 或连不上 | 默认只绑 127.0.0.1 |
| 旧进程占端口 | 启动失败 `address in use` | 先 `ps aux \| grep MCP-DM` 清干净 |


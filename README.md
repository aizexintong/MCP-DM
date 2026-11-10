# MCP-DM

[![Release](https://github.com/aizexintong/MCP-DM/actions/workflows/release.yml/badge.svg)](https://github.com/aizexintong/MCP-DM/actions/workflows/release.yml)
[![License: GPL-3.0](https://img.shields.io/badge/license-GPL--3.0-blue.svg)](LICENSE)
![Platform](https://img.shields.io/badge/platform-linux%20%7C%20windows-lightgrey.svg)

自托管的长期记忆 / 知识库。单个 Rust 二进制，同时提供 **Web 界面** 与 **MCP 服务**，数据存在**嵌入式 PostgreSQL** 里，整个数据目录可整体搬迁。

> 给 AI 用的安装与工具手册在 [AGENTS.md](AGENTS.md)，改代码的规范在 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 特性

- **单二进制**：Web + MCP + 数据库，不需要额外服务
- **零外部依赖**：CSS / JS / 前端库全部编译期内嵌，运行时不读文件、不连远程
- **默认只监听本机**：`BIND` 参数控制放行范围，从单机到内网一键切换
- **结构化的因果记录**：每条含 `起因 / 结果 / 分类 / 标签`，不是纯文本堆砌
- **检索而非搜索**：PostgreSQL `tsvector` 全文 + `pg_trgm` 模糊相似度混合打分
- **分层分类**：`ltree` 三级树，标识英文、名称中文
- **数据可搬迁**：`data/` 目录打包即走，不用重装数据库

---

## 快速开始

### 下载

从 [Releases](https://github.com/aizexintong/MCP-DM/releases) 取对应平台：

| 平台 | 文件 |
|---|---|
| Linux x86_64（静态，无依赖） | `MCP-DM-linux-amd64` |
| Windows x86_64 | `MCP-DM-windows-amd64.exe` |

### 运行

```bash
chmod +x MCP-DM-linux-amd64
./MCP-DM-linux-amd64
```

```powershell
.\MCP-DM-windows-amd64.exe
```

首次启动自动在 `./data/` 初始化数据库，**无需预装 PostgreSQL**。

```
网页  http://127.0.0.1:8767
MCP   http://127.0.0.1:8765/mcp
```

### 从源码构建

```bash
cargo build --release
./target/release/MCP-DM
```

静态构建（推荐，产物无 glibc 依赖）：

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

macOS：不提供预编译产物，需本地 `cargo build`（`postgresql_embedded` 缺 macOS 预编译包）。

---

## 访问控制

**默认只绑 `127.0.0.1`**，局域网访问需要显式指定 `BIND`。

一个参数同时表达「绑哪儿」和「谁能连」：

```bash
BIND=10.0.0.0/8 ./MCP-DM-linux-amd64    # 绑 0.0.0.0，仅放行内网 A 段
BIND=10.0.1.123 ./MCP-DM-linux-amd64    # 只绑这张网卡
BIND=0.0.0.0/0  ./MCP-DM-linux-amd64    # 不限制（公网暴露，谨慎）
```

| 写法 | 绑定地址 | 允许来源 |
|---|---|---|
| `127.0.0.1`（默认） | 127.0.0.1 | 仅本机 |
| `10.0.1.123` | 10.0.1.123 | 该网卡可达者 |
| `10.0.0.0/8` | 0.0.0.0 | 仅 10.0.0.0/8 |
| `0.0.0.0/0` 或 `0.0.0.0` | 0.0.0.0 | 不限制 |

原理：`bind()` 只能绑具体 IP，不能绑网段。所以网段写法绑 `0.0.0.0`，再按来源 IP 过滤，不在范围内返回 403。

---

## 配置

| 环境变量 | 默认 | 说明 |
|---|---|---|
| `BIND` | `127.0.0.1` | 绑定地址 / 放行网段（见上） |
| `DATA_DIR` | `./data` | 数据目录（PG 数据 + socket） |
| `MCP_PORT` | `8765` | MCP 端点端口 |
| `WEB_PORT` | `8767` | 网页端口 |
| `PG_PORT` | `55432` | 内嵌 PG 端口（仅 Unix socket，不监听 TCP） |
| `PG_PASSWORD` | `memory_local_only` | PostgreSQL 口令 |
| `RUST_LOG` | `info` | 日志级别 |

### 数据目录

```
data/
└─ pg/                    # PostgreSQL 数据
   ├─ base/
   ├─ global/
   ├─ pg_hba.conf
   └─ run/.s.PGSQL.55432  # Unix socket
```

**整个 `data/` 打包即可迁到新机器。**

---

## MCP 接入

端点：`http://127.0.0.1:8765/mcp`（streamable-http）

```json
{
  "mcpServers": {
    "memory": { "url": "http://127.0.0.1:8765/mcp" }
  }
}
```

八个工具：

| 工具 | 用途 |
|---|---|
| `mem_search` | 检索（全文 + 模糊混合） |
| `mem_get` | 读条目全文与引用关系 |
| `mem_add` | 写入新条目 |
| `mem_categories` | 分类树 |
| `mem_timeline` | 时间线 |
| `mem_stats` | 规模统计 |
| `mem_mark` | 收藏 / 精选 |
| `mem_like` | 点赞 |

参数与调用策略见 [AGENTS.md](AGENTS.md)。

---

## 技术栈

| 层 | 选型 |
|---|---|
| Web | `actix-web 4` |
| MCP | `rmcp 3.9`（streamable-http，`axum` 承载） |
| 数据库 | `sqlx 0.8` + PostgreSQL |
| 内嵌 PG | `postgresql_embedded 0.22`（rustls，随主进程启停） |
| 前端 | 原生 JS + CSS，`marked.js` 渲染 Markdown |
| 样式 | 全 CSS 变量驱动，暗色同步 |

---

## 架构

```
MCP-DM（单进程）
├─ Actix-web        :8767   网页 + REST API
├─ rmcp + axum      :8765   MCP（POST /mcp）
└─ PostgreSQL       子进程    Unix socket，随主进程启停
```

Rust 模块职责、数据库结构、API 契约见 [CONTRIBUTING.md](CONTRIBUTING.md)。

---

## 发布

版本号在本地递增，CI 只读不写（保证提交历史里只有作者本人）：

```bash
./scripts/release.sh          # 1.0.0 → 1.0.1
git push origin main --follow-tags
```

推 tag 后 GitHub Actions 自动构建 linux-amd64（静态）+ windows-amd64 并发布 Release。

---

## 许可

GPL-3.0，见 [LICENSE](LICENSE)。

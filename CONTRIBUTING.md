# 改动规范

面向要修改这个项目的人（和 AI）。安装使用见 [AGENTS.md](AGENTS.md)，项目介绍见 [README.md](README.md)。

---

## 绝对不能做

| ❌ 禁止 | 原因 |
|---|---|
| 引入 CDN 外链 | 要求离线可运行，所有资源走 `/static/` |
| 引入 npm / 前端构建 | 前端是原生 JS + CSS，零构建 |
| 硬编码颜色 / 尺寸 | 所有值必须来自 `web/css/app.css` 的 `:root` 变量 |
| 删 `web/vendor/` | `include_str!` 编译时需要 |
| 改完前端只刷浏览器 | 资源编译期内嵌，必须 `cargo build` 重启 |
| 用 `pkill -f "pattern"` | 会杀掉自己的 shell，用 PID 或 systemctl |
| 提交 `data/` | 含真实数据，已在 `.gitignore` |
| 在 `queries.rs` 之外写 SQL | 所有 SQL 集中管理 |
| 一个按钮循环多个状态 | 用户不知道点到了什么，每个状态一个按钮 |
| 用 `prompt()` / `confirm()` / `alert()` | 已有自定义 `<dialog>` 替代 |

---

## 目录职责

### Rust

| 文件 | 只做这一件事 |
|---|---|
| `src/main.rs` | 装配：起 PG、跑迁移、起两个 HTTP server |
| `src/config.rs` | 环境变量 → `Config` |
| `src/db.rs` | 内嵌 PG 启动、连接池、迁移 |
| `src/models.rs` | 数据结构 + `sqlx::FromRow`（**改 API 结构改这里**） |
| `src/queries.rs` | **全部 SQL** |
| `src/handlers.rs` | Actix 路由 + 静态资源内嵌表 |
| `src/mcp.rs` | rmcp `ServerHandler` |
| `src/render.rs` | 纯文本 → Markdown 转换（当前未被调用） |

### 前端

| 文件 | 职责 |
|---|---|
| `web/index.html` | 骨架（sidebar / topbar / 3 个 `<dialog>`） |
| `web/css/app.css` | **全部样式**，顶部 `:root` 是唯一色彩来源 |
| `web/js/api.js` | 接口封装 + 字段归一化 |
| `web/js/app.js` | 路由、渲染、事件委托 |

脚本加载顺序必须保持：`marked → api → app`

---

## 数据模型

```sql
categories (path ltree, label text, parent_path ltree)
entries    (id, title, body, cause, effect, cat_path ltree,
            tags text[], state, secrecy, search_vec tsvector)
links      (from_id, to_id)
marks      (entry_id, is_fav, is_pick, likes)
hotness    (entry_id, hits)
```

### 约定

1. **`cat_path` 是 `ltree`**
   - 前缀查询用 `<@`：`cat_path <@ 'mc-server'::ltree`
   - **不能**用 `LIKE`，**不能**在参数里拼 `.%`（报语法错）

2. **`path` 英文、`label` 中文**
   - 存 `path="mc-server.mods"`、`label="模组"`
   - 前端用 `/api/list` 建映射表翻译

3. **`state`**：`current` / `archived` / `obsolete`

4. **`secrecy`**：`public` / `internal` / `secret`
   - 所有查询必须带 `secrecy='public'`

5. **`mark_level` 是视图算的，不存表**
   ```sql
   CASE WHEN is_fav AND is_pick THEN 3
        WHEN is_pick THEN 2
        WHEN is_fav THEN 1
        ELSE 0 END
   ```

---

## API 响应外层（不统一，别假设）

| 端点 | 外层键 |
|---|---|
| `GET /api/list` | `{ categories }` |
| `GET /api/entry/:id` | **扁平对象** |
| `GET /api/search` | `{ status, query, total, results }` |
| `GET /api/timeline` | `{ timeline }` |
| `GET /api/stats` | `{ cats, entries, links, marked }` |

### 字段名

| 正确 | 错误 |
|---|---|
| `cat_path` | ~~category~~ |
| `backlinks` | ~~links_count~~ |
| `likes` | ~~like_count~~ |
| `body` | ~~content~~ |
| `state` | ~~status~~ |

---

## 搜索 SQL（最易改坏）

```sql
WHERE ($1 = '' OR search_vec @@ plainto_tsquery('simple', $1)
       OR title % $1 OR cause % $1 OR effect % $1 OR body % $1)
  AND ($2::ltree IS NULL OR cat_path <@ $2::ltree)
  AND secrecy = 'public'
ORDER BY base_score * (1.0 + 0.5 * mark_level) DESC
```

- `$1` 为空时**跳过全文匹配**，只按分类筛选（分类页依赖）
- `%` 是 `pg_trgm` 相似度，不是取模
- 已知缺陷：中文按标点切词，靠 trgm 兜底

---

## 前端约定

### 色彩

```css
:root {
  --bg --glass-top --glass-side --glass-card --glass-inner --glass-sink
  --text --muted --subtle
  --accent --accent-deep --accent-soft
  --line --glass-line --halo
  --shadow --shadow-hover
  --principle-bg --card-veil
}
:root[data-theme="dark"] { /* 同名换值 */ }
```

加组件时先找现成变量，没有就加变量，**不写死颜色**。

### 路由

path 路由（`history.pushState`），后端对非 `/api/*`、非 `/static/*` 返回 `index.html`。

### 交互

- 事件**委托**（内容是 `innerHTML` 注入的）
- 操作用 `data-*` 标记，不用内联 `onclick`
- 弹窗用原生 `<dialog>` + `showModal()`

---

## 构建

```bash
cargo build --release                                    # Linux glibc
cargo build --release --target x86_64-unknown-linux-musl # Linux 静态
cargo build --release --target x86_64-pc-windows-gnu     # Windows
```

Windows 交叉需要 mingw + `.cargo/config.toml` 指定 linker。

**macOS 不做交叉编译**（缺 Apple SDK + PG 预编译包）。

---

## 发布

推送到 `main` 即触发 [`.github/workflows/release.yml`](.github/workflows/release.yml)：

1. 自动 `1.0.x → 1.0.(x+1)`
2. 构建 linux-amd64（静态）+ windows-amd64
3. 创建 tag `v1.0.x` 和 Release
4. 校验资产齐全

不想触发就把改动标成 `[skip ci]`。


---

## 命名约定

三个名字各管一段，改一处要连带改全部，否则产物名、服务名、文档会对不上。

| 位置 | 值 | 作用 |
|---|---|---|
| `Cargo.toml` → `[package] name` | `mcp-dm` | **crate 名，必须全小写**，Rust 不允许大写 |
| `Cargo.toml` → `[[bin]] name` | `MCP-DM` | **产物文件名**，打包后的可执行文件就叫这个 |
| `Cargo.lock` → `[[package]] name` | `mcp-dm` | 与 package 一致 |

产物路径：
```
target/release/MCP-DM                     Linux glibc
target/x86_64-unknown-linux-musl/release/MCP-DM    Linux 静态
target/x86_64-pc-windows-gnu/release/MCP-DM.exe    Windows
```

### 改名要同步的地方

```bash
grep -rn "MCP-DM\|memory-kb" \
  Cargo.toml Cargo.lock \
  .github/workflows/release.yml \
  scripts/release.sh \
  README.md AGENTS.md CONTRIBUTING.md
```

清单：

- [ ] `Cargo.toml` 的 `[package] name`（小写）
- [ ] `Cargo.toml` 的 `[[bin]] name`（可大写）
- [ ] `Cargo.lock` 的 `[[package]] name`
- [ ] `.github/workflows/release.yml` 的 `env.BIN_NAME`
- [ ] `scripts/release.sh` 里的 `cargo update -p <name>`
- [ ] 三份文档里的可执行命令
- [ ] 本机 systemd 的 `ExecStart`
- [ ] Release 附件命名

---

## 构建期版本注入

`build.rs` 把版本号写进 `env!("APP_VERSION")`：

```
本地构建   取 Cargo.toml 的 version
CI 构建    取 BUILD_VERSION 环境变量（由 git tag 推导）
```

好处：**CI 不需要修改任何文件就能打出正确版本**，仓库历史里不会出现机器提交
（否则 Contributors 列表会多出一个 `github-actions[bot]`）。

验证：
```bash
BUILD_VERSION=9.9.9 cargo build --release
./target/release/MCP-DM     # 启动日志第一行应显示 v9.9.9
```

---

## 踩过的坑（都是真出过的问题）

| 现象 | 根因 | 修法 |
|---|---|---|
| CI 全程失败，报 Cargo.lock 解析错 | rebase 冲突标记 `<<<<<<<` 被提交进 `Cargo.lock` | 别手改 lock；`cargo update` 或删掉重新生成 |
| Linux 构建失败：不是静态链接 | musl 静态包的 `ldd` 输出是 `statically linked`，而检查找的是 `not a dynamic executable` | 判断输出里有没有 `=>`，不依赖措辞 |
| CI 报源码有警告但本地没有 | 门禁 `grep "^warning:"` 连依赖 crate 的警告也算 | 加 `grep -A2 ... \| grep "--> src/"` 限定本项目 |
| 改了前端页面没变化 | 资源是 `include_str!` 编译期内嵌 | 必须 `cargo build` 重启，光刷浏览器没用 |
| `sort` 参数怎么调都没用 | `ORDER BY` 硬编码在 SQL 里，`sort` 变量算了但没用上 | 白名单生成排序子句，`format!` 拼进 SQL |
| 设 `BIND=10.0.0.0/8` 后本机 curl 403 | 回环地址不在网段白名单里 | `permits()` 里回环直接放行 |
| `/8` 显示成 `/24` | `(!mask).leading_zeros()` 算的是前导零 | 改用 `mask.count_ones()` |
| 删分类后条目失踪 | 兜底分类路径散落 4 处硬编码 | 提成 `FALLBACK_CATEGORY` 常量 |
| 收藏/精选只有一个按钮 | `mark_level = (n+1) % 4` 循环 | 拆成两个独立开关 + 点赞 |
| 编辑器滚不动 | `x-data` 嵌套覆盖了外层作用域，按钮全失效 | 状态和方法放在同一个 `x-data` 里 |
| 页面底部按钮看不见 | 操作放在内容末尾 | 改成吸顶工具条 `position: sticky` |
| 深色模式露出白块 | 组件用了硬编码 `rgba(255,255,255,.7)` | 全部改 CSS 变量，暗色只改变量值 |

---

## 排查手法

**别猜，去测。** 这个项目踩的坑里，一半是「以为改好了其实没生效」。

前端用无头浏览器验证：

```js
const { chromium } = require('playwright-core');
const p = await (await br.newContext({ viewport:{width:1600,height:950} })).newPage();
await p.goto('http://127.0.0.1:8767/entry/2', { waitUntil:'networkidle' });
// 测量真实布局，不要凭截图猜
const m = await p.evaluate(() => {
  const el = document.querySelector('.editor');
  const r = el.getBoundingClientRect();
  return { h: Math.round(r.height), overflow: getComputedStyle(el).overflowY };
});
```

后端直接打接口看真实返回，不要读代码推测：

```bash
curl -s http://127.0.0.1:8767/api/list | python3 -m json.tool | head -20
curl -s "http://127.0.0.1:8767/api/search?sort=newest&limit=3"
```

确认二进制版本是不是线上那一份：

```bash
grep "MCP-DM v" /data/projects/memory/logs/kb.log | tail -1
```

---

## 检查清单

- [ ] `cargo build --release` 通过
- [ ] 无新增 CDN / 外链
- [ ] 无硬编码颜色：`grep -E '#[0-9a-f]{3,6}|rgba\(' web/css/`
- [ ] 新增 SQL 带 `secrecy='public'`
- [ ] 字段名对照上面
- [ ] 暗色模式验证
- [ ] 移动端（<768px）不溢出
- [ ] `web/vendor/` 三个文件都在
- [ ] 没把 `data/` / `target/` 加进 git

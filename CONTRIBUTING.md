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

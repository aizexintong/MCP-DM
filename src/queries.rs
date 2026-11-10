//! SQL 查询层：所有数据库操作集中在这里
use crate::models::*;
use anyhow::Result;
use sqlx::PgPool;

// ============================================================================
// 约定常量 —— 需要调整分类结构时只改这里
// ============================================================================

/// 删除分类时，无家可归的条目落到这里。
/// 若改名，必须保证该路径存在（`delete_category` 会自动创建）。
pub const FALLBACK_CATEGORY: &str = "misc-root.unsorted";

/// 搜索打分权重：全文命中 vs 模糊相似度
const W_FULLTEXT: f64 = 0.6;
const W_TRIGRAM: f64 = 0.4;

/// 标记加成：每条收藏/精选提升的分数比例
const MARK_BOOST: f64 = 0.5;

/// 列出分类 + 该分类直属的条目（一次取全，前端不再二次请求）
pub async fn list_tree(pool: &PgPool) -> Result<Vec<serde_json::Value>> {
    let rows: Vec<(String, String, i32, i64)> = sqlx::query_as(
        r#"
        SELECT c.path::text, c.label, nlevel(c.path),
               (SELECT COUNT(*) FROM entries e
                WHERE e.cat_path <@ c.path AND e.secrecy='public') AS cnt
        FROM categories c
        ORDER BY c.path
        "#,
    ).fetch_all(pool).await?;

    let mut out = Vec::new();
    for (path, label, depth, count) in rows {
        let items: Vec<(i64, String, i32, String)> = sqlx::query_as(
            r#"SELECT id, title, mark_level, cat_path::text FROM entries_full
               WHERE cat_path = $1::ltree AND secrecy='public'
               ORDER BY mark_level DESC, updated_at DESC LIMIT 200"#,
        ).bind(&path).fetch_all(pool).await.unwrap_or_default();

        out.push(serde_json::json!({
            "path": path, "label": label, "depth": depth, "count": count,
            "items": items.iter().map(|(id,t,m,cp)| serde_json::json!({
                "id": id, "title": t, "mark_level": m, "cat_path": cp
            })).collect::<Vec<_>>()
        }));
    }
    Ok(out)
}

/// 列出所有分类及其条目数
pub async fn list_categories(pool: &PgPool) -> Result<Vec<CategoryNode>> {
    let rows = sqlx::query_as::<_, CategoryNode>(
        r#"
        SELECT
            c.path::text                       AS path,
            c.label                            AS label,
            nlevel(c.path)                     AS depth,
            COUNT(e.id)                        AS count
        FROM categories c
        LEFT JOIN entries e ON e.cat_path <@ c.path AND e.secrecy = 'public'
        GROUP BY c.path, c.label
        ORDER BY c.path
        "#,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 确保分类路径存在（逐级创建）
pub async fn ensure_category(pool: &PgPool, cat_path: &str) -> Result<()> {
    let path = cat_path.replace('/', ".");
    let parts: Vec<&str> = path.split('.').collect();
    for i in 1..=parts.len() {
        let sub = parts[..i].join(".");
        let label = parts[i - 1];
        let parent = if i > 1 { Some(parts[..i - 1].join(".")) } else { None };
        sqlx::query(
            r#"INSERT INTO categories (path, label, parent_path, sort_order)
               VALUES ($1::ltree, $2, $3::ltree, 0)
               ON CONFLICT (path) DO NOTHING"#,
        )
        .bind(&sub)
        .bind(label)
        .bind(&parent)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// 获取条目详情
pub async fn get_entry(pool: &PgPool, id: i64) -> Result<Option<Entry>> {
    let row = sqlx::query_as::<_, Entry>(
        r#"SELECT id,title,body,cause,effect,cat_path::text AS cat_path,tags,source,state,state_reason,secrecy,inferred,created_at,updated_at,is_fav,is_pick,likes,hits,mark_level,backlinks FROM entries_full WHERE id = $1"#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// 出链
pub async fn get_links_out(pool: &PgPool, id: i64) -> Result<Vec<LinkRef>> {
    let rows = sqlx::query_as::<_, LinkRef>(
        r#"SELECT e.id, e.title FROM links l JOIN entries e ON e.id = l.to_id WHERE l.from_id = $1"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 入链（被引用）
pub async fn get_links_in(pool: &PgPool, id: i64) -> Result<Vec<LinkRef>> {
    let rows = sqlx::query_as::<_, LinkRef>(
        r#"SELECT e.id, e.title FROM links l JOIN entries e ON e.id = l.from_id WHERE l.to_id = $1"#,
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 全文 + 模糊混合搜索
///
/// 排序权重：
///   ts_rank (全文) × 0.6 + similarity (trgm) × 0.4
///   再乘：标记加成(1~2.5) × 热度加成(1~1.5) × 时效衰减
pub async fn search(
    pool: &PgPool,
    q: &str,
    limit: i64,
    sort: &str,
    cat: Option<&str>,
    days: Option<i64>,
    state: Option<&str>,
) -> Result<Vec<SearchHit>> {
    let cat_path = cat.map(|c| c.replace('/', "."));

    // ORDER BY 来自白名单，不接受外部字符串拼接
    let order_by = match sort {
        "newest" => "updated_at DESC",
        "marked" => "mark_level DESC, final_score DESC",
        _ => "final_score DESC",
    };

    let sql = format!(
        r#"
        WITH scored AS (
            SELECT
                e.id, e.title, e.cat_path::text AS cat_path, e.body, e.cause, e.effect,
                e.state, e.mark_level, e.updated_at,
                (CASE WHEN $1 = '' THEN 0 ELSE
                    ts_rank(e.search_vec, plainto_tsquery('simple', $1)) * {W_FULLTEXT}
                    + GREATEST(
                        similarity(e.title, $1),
                        similarity(e.cause, $1),
                        similarity(e.effect, $1)
                      ) * {W_TRIGRAM}
                END) AS base_score
            FROM entries_full e
            WHERE
                -- $1 为空时只按分类/状态筛选，不做全文匹配
                ($1 = ''
                 OR e.search_vec @@ plainto_tsquery('simple', $1)
                 OR e.title % $1 OR e.cause % $1 OR e.effect % $1 OR e.body % $1)
                AND ($2::ltree IS NULL OR e.cat_path <@ $2::ltree)
                AND ($3::bigint IS NULL OR e.updated_at > now() - ($3 || ' days')::interval)
                AND ($4::text IS NULL OR e.state = $4)
                AND e.secrecy = 'public'
        )
        SELECT
            id, title, cat_path, cause, effect, state, mark_level, updated_at,
            (base_score * (1.0 + {MARK_BOOST} * mark_level))::real AS final_score,
            (base_score * (1.0 + {MARK_BOOST} * mark_level))::real AS score,
            LEFT(body, 200) AS snippet
        FROM scored
        ORDER BY {order_by}
        LIMIT $5
        "#
    );
    let rows = sqlx::query_as::<_, SearchHit>(&sql)
    .bind(q)
    .bind(&cat_path)
    .bind(days)
    .bind(state)
    .bind(limit)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// 新建条目
pub async fn create_entry(
    pool: &PgPool,
    title: &str,
    body: &str,
    cause: &str,
    effect: &str,
    cat_path: &str,
    tags: &[String],
    source: &[String],
) -> Result<i64> {
    ensure_category(pool, cat_path).await?;
    let path = cat_path.replace('/', ".");
    let id: i64 = sqlx::query_scalar(
        r#"INSERT INTO entries (title, body, cause, effect, cat_path, tags, source)
           VALUES ($1, $2, $3, $4, $5::ltree, $6, $7) RETURNING id"#,
    )
    .bind(title)
    .bind(body)
    .bind(cause)
    .bind(effect)
    .bind(&path)
    .bind(tags)
    .bind(source)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

/// 更新条目（只更新 Some 的字段）
pub async fn update_entry(pool: &PgPool, id: i64, u: &UpdateEntry) -> Result<bool> {
    if let Some(cp) = &u.cat_path {
        ensure_category(pool, cp).await?;
    }
    let cat = u.cat_path.as_ref().map(|c| c.replace('/', "."));
    let r = sqlx::query(
        r#"UPDATE entries SET
            title       = COALESCE($2, title),
            body        = COALESCE($3, body),
            cause       = COALESCE($4, cause),
            effect      = COALESCE($5, effect),
            cat_path    = COALESCE($6::ltree, cat_path),
            state       = COALESCE($7, state),
            state_reason= COALESCE($8, state_reason),
            tags        = COALESCE($9, tags)
           WHERE id = $1"#,
    )
    .bind(id)
    .bind(&u.title)
    .bind(&u.body)
    .bind(&u.cause)
    .bind(&u.effect)
    .bind(&cat)
    .bind(&u.state)
    .bind(&u.state_reason)
    .bind(&u.tags)
    .execute(pool)
    .await?;
    Ok(r.rows_affected() > 0)
}

/// 删除条目
pub async fn delete_entry(pool: &PgPool, id: i64) -> Result<bool> {
    let r = sqlx::query("DELETE FROM entries WHERE id = $1")
        .bind(id).execute(pool).await?;
    Ok(r.rows_affected() > 0)
}

/// 设置标记
pub async fn set_mark(pool: &PgPool, id: i64, level: i32) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO marks (entry_id, is_fav, is_pick, updated_at)
           VALUES ($1, $2, $3, now())
           ON CONFLICT (entry_id) DO UPDATE SET
             is_fav = $2, is_pick = $3, updated_at = now()"#,
    )
    .bind(id)
    .bind(level == 1 || level == 3)
    .bind(level == 2 || level == 3)
    .execute(pool)
    .await?;
    Ok(())
}

/// 点赞
pub async fn like(pool: &PgPool, id: i64, delta: i32) -> Result<i32> {
    let n: i32 = sqlx::query_scalar(
        r#"INSERT INTO marks (entry_id, likes) VALUES ($1, GREATEST(0, $2))
           ON CONFLICT (entry_id) DO UPDATE SET
             likes = GREATEST(0, marks.likes + $2), updated_at = now()
           RETURNING likes"#,
    )
    .bind(id).bind(delta).fetch_one(pool).await?;
    Ok(n)
}

/// 记录命中热度
pub async fn record_hits(pool: &PgPool, ids: &[i64]) -> Result<()> {
    for id in ids {
        sqlx::query(
            r#"INSERT INTO hotness (entry_id, hits, last_hit) VALUES ($1, 1, now())
               ON CONFLICT (entry_id) DO UPDATE SET
                 hits = hotness.hits + 1, last_hit = now()"#,
        )
        .bind(id).execute(pool).await.ok();
    }
    Ok(())
}

/// 时间线
pub async fn timeline(
    pool: &PgPool,
    cat: Option<&str>,
    limit: i64,
) -> Result<Vec<TimelineItem>> {
    let cat_path = cat.map(|c| c.replace('/', "."));
    let rows = sqlx::query_as::<_, TimelineItem>(
        r#"
        SELECT id, title, cat_path::text AS cat_path, updated_at, cause, effect, mark_level
        FROM entries_full
        WHERE secrecy = 'public'
          AND ($1::ltree IS NULL OR cat_path <@ $1::ltree)
        ORDER BY updated_at DESC
        LIMIT $2
        "#,
    )
    .bind(&cat_path)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// 统计
pub async fn stats(pool: &PgPool) -> Result<(i64, i64, i64, i64)> {
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM entries WHERE secrecy='public'")
        .fetch_one(pool).await?;
    let cats: i64 = sqlx::query_scalar("SELECT COUNT(DISTINCT cat_path) FROM entries")
        .fetch_one(pool).await?;
    let links: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM links")
        .fetch_one(pool).await?;
    let marked: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM marks WHERE is_fav OR is_pick")
        .fetch_one(pool).await?;
    Ok((total, cats, links, marked))
}

// ---------- 分类管理 ----------

#[derive(Debug, serde::Deserialize)]
pub struct CatOp {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub new_path: Option<String>,
    #[serde(default)]
    pub new_label: Option<String>,
}

/// 改名（label 或 path）
pub async fn rename_category(pool: &PgPool, old: &str, new_path: Option<&str>, new_label: Option<&str>) -> Result<()> {
    let mut tx = pool.begin().await?;

    if let Some(np) = new_path {
        let np = np.replace('/', ".");
        // 更新分类本身
        sqlx::query("UPDATE categories SET path=$2::ltree, label=$3, parent_path=$4::ltree WHERE path=$1::ltree")
            .bind(old).bind(&np)
            .bind(new_label.unwrap_or_else(|| np.split('.').last().unwrap_or("")))
            .bind(np.rsplit_once('.').map(|(p, _)| p.to_string()))
            .execute(&mut *tx).await?;
        // 更新条目
        sqlx::query("UPDATE entries SET cat_path=$2::ltree WHERE cat_path=$1::ltree")
            .bind(old).bind(&np).execute(&mut *tx).await?;
        // 更新子分类：用 ltree 的 subpath 替换
        sqlx::query(r#"
            UPDATE categories SET
              path = $2::ltree || subpath(path, nlevel($1::ltree)),
              parent_path = CASE WHEN parent_path IS NULL THEN NULL
                                 ELSE $2::ltree || subpath(parent_path, nlevel($1::ltree)) END
            WHERE path <@ $1::ltree AND path <> $1::ltree"#)
            .bind(old).bind(&np).execute(&mut *tx).await?;
        sqlx::query(r#"UPDATE entries SET cat_path = $2::ltree || subpath(cat_path, nlevel($1::ltree))
                       WHERE cat_path <@ $1::ltree AND cat_path <> $1::ltree"#)
            .bind(old).bind(&np).execute(&mut *tx).await?;
    } else if let Some(nl) = new_label {
        sqlx::query("UPDATE categories SET label=$2 WHERE path=$1::ltree")
            .bind(old).bind(nl).execute(&mut *tx).await?;
    }

    tx.commit().await?;
    Ok(())
}

/// 删除分类（含子分类），条目移到 misc-root.unsorted
pub async fn delete_category(pool: &PgPool, path: &str) -> Result<i64> {
    let mut tx = pool.begin().await?;
    ensure_category(pool, FALLBACK_CATEGORY).await.ok();

    let moved = sqlx::query("UPDATE entries SET cat_path=$2::ltree WHERE cat_path=$1::ltree")
        .bind(path).bind(FALLBACK_CATEGORY).execute(&mut *tx).await?.rows_affected();

    // 子分类的条目也移走
    sqlx::query("UPDATE entries SET cat_path=$2::ltree WHERE cat_path <@ $1::ltree")
        .bind(path).bind(FALLBACK_CATEGORY).execute(&mut *tx).await.ok();

    sqlx::query("DELETE FROM categories WHERE path <@ $1::ltree")
        .bind(path).execute(&mut *tx).await?;

    tx.commit().await?;
    Ok(moved as i64)
}

/// 新增分类（自动创建父级）
pub async fn create_category(pool: &PgPool, cat_path: &str, label: Option<&str>) -> Result<String> {
    let path = cat_path.replace('/', ".").trim_matches('.').to_string();
    let label = label.map(|s| s.to_string())
        .unwrap_or_else(|| path.rsplit('.').next().unwrap_or(&path).to_string());
    ensure_category(pool, &path).await?;
    // 若给了自定义 label，更新之
    if label != *path.rsplit('.').next().unwrap_or("") {
        sqlx::query("UPDATE categories SET label=$2 WHERE path=$1::ltree")
            .bind(&path).bind(&label).execute(pool).await?;
    }
    Ok(path)
}

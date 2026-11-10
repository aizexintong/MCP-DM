-- ============================================================
-- 知识库表结构
-- ============================================================

CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS ltree;

-- ---- 分类树（ltree 天然支持层级）----
CREATE TABLE categories (
    id          BIGSERIAL PRIMARY KEY,
    path        ltree NOT NULL UNIQUE,          -- MC服务器.变更历史.2026-10月
    label       TEXT NOT NULL,                  -- 2026-10月
    parent_path ltree,
    sort_order  INT DEFAULT 0,
    created_at  TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX idx_categories_path ON categories USING GIST (path);
CREATE INDEX idx_categories_parent ON categories (parent_path);

-- ---- 条目 ----
CREATE TABLE entries (
    id          BIGSERIAL PRIMARY KEY,
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',
    cause       TEXT NOT NULL DEFAULT '',
    effect      TEXT NOT NULL DEFAULT '',
    cat_path    ltree NOT NULL,                 -- 关联 categories.path
    tags        TEXT[] DEFAULT '{}',
    source      TEXT[] DEFAULT '{}',
    state       TEXT NOT NULL DEFAULT 'current',  -- current/archived/obsolete
    state_reason TEXT DEFAULT '',
    secrecy     TEXT NOT NULL DEFAULT 'public',   -- public/internal/secret
    inferred    BOOLEAN DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),

    -- 全文检索向量（中文用 simple 配置 + trgm 辅助）
    search_vec  tsvector
);
CREATE INDEX idx_entries_cat ON entries USING GIST (cat_path);
CREATE INDEX idx_entries_updated ON entries (updated_at DESC);
CREATE INDEX idx_entries_state ON entries (state);
CREATE INDEX idx_entries_vector ON entries USING GIN (search_vec);
CREATE INDEX idx_entries_title_trgm ON entries USING GIN (title gin_trgm_ops);
CREATE INDEX idx_entries_body_trgm ON entries USING GIN (body gin_trgm_ops);
CREATE INDEX idx_entries_tags ON entries USING GIN (tags);

-- ---- 引用关系 ----
CREATE TABLE links (
    from_id  BIGINT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    to_id    BIGINT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    PRIMARY KEY (from_id, to_id)
);
CREATE INDEX idx_links_to ON links (to_id);

-- ---- 标记：收藏/精选/点赞 ----
CREATE TABLE marks (
    entry_id  BIGINT PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
    is_fav    BOOLEAN DEFAULT FALSE,
    is_pick   BOOLEAN DEFAULT FALSE,
    likes     INT DEFAULT 0,
    updated_at TIMESTAMPTZ DEFAULT now()
);

-- ---- 热度：检索命中次数 ----
CREATE TABLE hotness (
    entry_id  BIGINT PRIMARY KEY REFERENCES entries(id) ON DELETE CASCADE,
    hits      BIGINT DEFAULT 0,
    last_hit  TIMESTAMPTZ DEFAULT now()
);

-- ---- 检索日志（用于去重和统计）----
CREATE TABLE search_log (
    id        BIGSERIAL PRIMARY KEY,
    query     TEXT NOT NULL,
    qhash     TEXT NOT NULL,
    hits      INT DEFAULT 0,
    created_at TIMESTAMPTZ DEFAULT now()
);
CREATE INDEX idx_search_log_hash ON search_log (qhash, created_at DESC);

-- ============================================================
-- 触发器：自动更新 search_vec
-- ============================================================
CREATE OR REPLACE FUNCTION update_search_vec() RETURNS trigger AS $$
BEGIN
    NEW.search_vec :=
        setweight(to_tsvector('simple', coalesce(NEW.title,'')), 'A') ||
        setweight(to_tsvector('simple', coalesce(NEW.cause,'')), 'B') ||
        setweight(to_tsvector('simple', coalesce(NEW.effect,'')), 'B') ||
        setweight(to_tsvector('simple', coalesce(NEW.body,'')), 'C');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_entries_search_vec
    BEFORE INSERT OR UPDATE OF title, cause, effect, body ON entries
    FOR EACH ROW EXECUTE FUNCTION update_search_vec();

-- 自动更新 updated_at
CREATE OR REPLACE FUNCTION touch_updated_at() RETURNS trigger AS $$
BEGIN
    NEW.updated_at := now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_entries_touch
    BEFORE UPDATE ON entries
    FOR EACH ROW EXECUTE FUNCTION touch_updated_at();

-- ============================================================
-- 视图：带标记和热度的条目
-- ============================================================
CREATE VIEW entries_full AS
SELECT
    e.*,
    COALESCE(m.is_fav, FALSE) AS is_fav,
    COALESCE(m.is_pick, FALSE) AS is_pick,
    COALESCE(m.likes, 0) AS likes,
    COALESCE(h.hits, 0) AS hits,
    CASE
        WHEN COALESCE(m.is_fav, FALSE) AND COALESCE(m.is_pick, FALSE) THEN 3
        WHEN COALESCE(m.is_pick, FALSE) THEN 2
        WHEN COALESCE(m.is_fav, FALSE) THEN 1
        ELSE 0
    END AS mark_level,
    (SELECT COUNT(*) FROM links l WHERE l.to_id = e.id) AS backlinks
FROM entries e
LEFT JOIN marks m ON m.entry_id = e.id
LEFT JOIN hotness h ON h.entry_id = e.id;

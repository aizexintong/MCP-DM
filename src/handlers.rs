//! Actix-web 路由与处理函数
use crate::access::Allowed;
use crate::models::*;
use crate::queries;
use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.route("/", web::get().to(index))
        .route("/api/list", web::get().to(api_list))
        .route("/api/stats", web::get().to(api_stats))
        .route("/api/search", web::get().to(api_search))
        .route("/api/timeline", web::get().to(api_timeline))
        .route("/api/entry/{id}", web::get().to(api_entry_get))
        .route("/api/entry/{id}", web::post().to(api_entry_update))
        .route("/api/entry/{id}", web::delete().to(api_entry_delete))
        .route("/api/new", web::post().to(api_new))
        .route("/api/mark/{id}", web::post().to(api_mark))
        .route("/api/like/{id}", web::post().to(api_like))
        .route("/api/category/rename", web::post().to(api_cat_rename))
        .route("/api/category/delete", web::post().to(api_cat_delete))
        .route("/api/category/create", web::post().to(api_cat_create))
        .route("/static/{path:.*}", web::get().to(static_file))
        .default_service(web::to(index));
}

// ---------- 静态资源 ----------
macro_rules! assets {
    ($($p:literal),* $(,)?) => {
        &[$(($p, include_str!(concat!("../web/", $p)))),*] as &[(&str, &str)]
    };
}
const ASSETS: &[(&str, &str)] = assets![
    "index.html",
    "css/app.css",
    "vendor/alpine.min.js",
    "vendor/alpine-collapse.min.js",
    "vendor/marked.min.js",
    "js/api.js",
    "js/app.js",
];

async fn index() -> impl Responder {
    let html = ASSETS.iter().find(|(p, _)| *p == "index.html").map(|(_, c)| *c).unwrap_or("");
    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .insert_header(("Cache-Control", "no-cache, must-revalidate"))
        .body(html)
}

async fn static_file(req: HttpRequest) -> impl Responder {
    let path = req.match_info().query("path");
    match ASSETS.iter().find(|(p, _)| *p == path) {
        Some((_, body)) => {
            let ct = if path.ends_with(".css") {
                "text/css; charset=utf-8"
            } else if path.ends_with(".js") {
                "application/javascript; charset=utf-8"
            } else {
                "text/plain; charset=utf-8"
            };
            HttpResponse::Ok()
                .content_type(ct)
                .insert_header(("Cache-Control", "no-cache, must-revalidate"))
                .body(*body)
        }
        None => HttpResponse::NotFound().body("not found"),
    }
}

// ---------- API ----------
async fn api_list(_: Allowed, pool: web::Data<Arc<PgPool>>) -> impl Responder {
    match queries::list_tree(&pool).await {
        Ok(cats) => HttpResponse::Ok().json(json!({ "categories": cats })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_stats(_: Allowed, pool: web::Data<Arc<PgPool>>) -> impl Responder {
    match queries::stats(&pool).await {
        Ok((total, cats, links, marked)) => HttpResponse::Ok().json(json!({
            "entries": total, "cats": cats, "links": links, "marked": marked
        })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_search(_: Allowed, pool: web::Data<Arc<PgPool>>, q: web::Query<SearchQuery>) -> impl Responder {
    // q 和 cat 都空时视为「浏览全部」（按 sort 排序），首页「标记优先」需要

    match queries::search(
        &pool,
        &q.q,
        q.limit.unwrap_or(30),
        q.sort.as_deref().unwrap_or("relevance"),
        q.cat.as_deref(),
        q.days,
        q.state.as_deref(),
    )
    .await
    {
        Ok(hits) => {
            let ids: Vec<i64> = hits.iter().take(20).map(|h| h.id).collect();
            let _ = queries::record_hits(&pool, &ids).await;
            HttpResponse::Ok().json(SearchResponse {
                status: "found".into(),
                query: q.q.clone(),
                total: hits.len() as i64,
                results: hits,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_timeline(_: Allowed, pool: web::Data<Arc<PgPool>>, q: web::Query<TimelineQuery>) -> impl Responder {
    match queries::timeline(&pool, q.cat.as_deref(), q.limit.unwrap_or(200)).await {
        Ok(items) => HttpResponse::Ok().json(json!({ "timeline": items })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_entry_get(_: Allowed, pool: web::Data<Arc<PgPool>>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match queries::get_entry(&pool, id).await {
        Ok(Some(e)) => {
            let out = queries::get_links_out(&pool, id).await.unwrap_or_default();
            let inn = queries::get_links_in(&pool, id).await.unwrap_or_default();
            // 正文原样返回，前端用同一套 Markdown 渲染器（marked.js）统一处理
            HttpResponse::Ok().json(EntryDetail { entry: e, html: String::new(), links_out: out, links_in: inn })
        }
        Ok(None) => HttpResponse::NotFound().json(json!({ "error": "not found" })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_entry_update(_: Allowed, 
    pool: web::Data<Arc<PgPool>>,
    path: web::Path<i64>,
    body: web::Json<UpdateEntry>,
) -> impl Responder {
    let id = path.into_inner();
    match queries::update_entry(&pool, id, &body).await {
        Ok(true) => HttpResponse::Ok().json(json!({ "ok": true })),
        Ok(false) => HttpResponse::NotFound().json(json!({ "error": "not found" })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_entry_delete(_: Allowed, pool: web::Data<Arc<PgPool>>, path: web::Path<i64>) -> impl Responder {
    let id = path.into_inner();
    match queries::delete_entry(&pool, id).await {
        Ok(true) => HttpResponse::Ok().json(json!({ "ok": true })),
        Ok(false) => HttpResponse::NotFound().json(json!({ "error": "not found" })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_new(_: Allowed, pool: web::Data<Arc<PgPool>>, body: web::Json<CreateEntry>) -> impl Responder {
    match queries::create_entry(
        &pool,
        &body.title, &body.body, &body.cause, &body.effect,
        &body.cat_path, &body.tags, &body.source,
    )
    .await
    {
        Ok(id) => HttpResponse::Ok().json(json!({ "ok": true, "id": id })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_mark(_: Allowed, pool: web::Data<Arc<PgPool>>, path: web::Path<i64>, body: web::Json<serde_json::Value>) -> impl Responder {
    let id = path.into_inner();
    let level = body.get("level").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    match queries::set_mark(&pool, id, level).await {
        Ok(_) => HttpResponse::Ok().json(json!({ "ok": true })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

async fn api_cat_create(_: Allowed, pool: web::Data<Arc<PgPool>>, b: web::Json<queries::CatOp>) -> impl Responder {
    if b.path.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({"error": "path required"}));
    }
    match queries::create_category(&pool, &b.path, b.label.as_deref()).await {
        Ok(p) => HttpResponse::Ok().json(json!({"ok": true, "path": p})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e.to_string()})),
    }
}

async fn api_cat_rename(_: Allowed, pool: web::Data<Arc<PgPool>>, b: web::Json<queries::CatOp>) -> impl Responder {
    match queries::rename_category(&pool, &b.path, b.new_path.as_deref(), b.new_label.as_deref()).await {
        Ok(_) => HttpResponse::Ok().json(json!({"ok": true})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e.to_string()})),
    }
}

async fn api_cat_delete(_: Allowed, pool: web::Data<Arc<PgPool>>, b: web::Json<queries::CatOp>) -> impl Responder {
    match queries::delete_category(&pool, &b.path).await {
        Ok(n) => HttpResponse::Ok().json(json!({"ok": true, "moved": n})),
        Err(e) => HttpResponse::InternalServerError().json(json!({"error": e.to_string()})),
    }
}

async fn api_like(_: Allowed, pool: web::Data<Arc<PgPool>>, path: web::Path<i64>, body: web::Json<serde_json::Value>) -> impl Responder {
    let id = path.into_inner();
    let delta = body.get("delta").and_then(|v| v.as_i64()).unwrap_or(1) as i32;
    match queries::like(&pool, id, delta).await {
        Ok(n) => HttpResponse::Ok().json(json!({ "ok": true, "likes": n })),
        Err(e) => HttpResponse::InternalServerError().json(json!({ "error": e.to_string() })),
    }
}

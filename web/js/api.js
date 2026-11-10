/* MCP-DM API adapter — fields and envelopes match the supplied live API contract. */
(() => {
  const BASE = window.MCP_DM_API_BASE || "";
  async function request(path, options = {}) {
    const response = await fetch(`${BASE}${path}`, {
      ...options,
      headers: { "Content-Type": "application/json", ...(options.headers || {}) }
    });
    const raw = await response.text();
    let data = null;
    try { data = raw ? JSON.parse(raw) : null; } catch { data = raw; }
    if (!response.ok) {
      const message = data && (data.error || data.message);
      throw new Error(message || `请求失败 (${response.status})`);
    }
    return data;
  }
  const json = (method, body) => ({ method, body: JSON.stringify(body) });
  const normalizeEntry = (raw = {}) => ({
    id: raw.id ?? "",
    title: raw.title ?? "无标题",
    cause: raw.cause ?? "",
    effect: raw.effect ?? "",
    body: raw.body ?? "",
    cat_path: raw.cat_path ?? "",
    state: raw.state ?? "current",
    state_reason: raw.state_reason ?? "",
    secrecy: raw.secrecy ?? "public",
    inferred: !!raw.inferred,
    source: Array.isArray(raw.source) ? raw.source : [],
    tags: Array.isArray(raw.tags) ? raw.tags : [],
    created_at: raw.created_at ?? "",
    updated_at: raw.updated_at ?? "",
    mark_level: Number(raw.mark_level) || 0,
    likes: Number(raw.likes) || 0,
    hits: Number(raw.hits) || 0,
    backlinks: Number(raw.backlinks) || 0,
    is_fav: !!raw.is_fav,
    is_pick: !!raw.is_pick,
    links_out: Array.isArray(raw.links_out) ? raw.links_out : [],
    links_in: Array.isArray(raw.links_in) ? raw.links_in : [],
    snippet: raw.snippet ?? "",
    score: raw.score ?? null,
    raw
  });
  const params = values => {
    const q = new URLSearchParams();
    Object.entries(values || {}).forEach(([k, v]) => {
      if (v !== "" && v !== null && v !== undefined) q.set(k, String(v));
    });
    return q.toString();
  };

  window.MCPApi = {
    normalizeEntry,
    async list() {
      const data = await request("/api/list");
      return Array.isArray(data?.categories) ? data.categories : [];
    },
    async stats() { return request("/api/stats"); },
    async entry(id) { return normalizeEntry(await request(`/api/entry/${encodeURIComponent(id)}`)); },
    async timeline(options = {}) {
      const data = await request(`/api/timeline?${params({ limit: options.limit ?? 300, cat: options.cat })}`);
      return (Array.isArray(data?.timeline) ? data.timeline : []).map(normalizeEntry);
    },
    async search(options = {}) {
      const query = params({
        q: options.q, cat: options.cat, sort: options.sort || "relevance",
        limit: options.limit ?? 30, days: options.days, state: options.state
      });
      const data = await request(`/api/search?${query}`);
      return (Array.isArray(data?.results) ? data.results : []).map(normalizeEntry);
    },
    async save(entry) {
      const payload = {
        title: entry.title,
        body: entry.body || "",
        cause: entry.cause || "",
        effect: entry.effect || "",
        cat_path: entry.cat_path,
        tags: Array.isArray(entry.tags) ? entry.tags : [],
        ...(entry.id ? { state: entry.state, state_reason: entry.state_reason || "" } : {}),
      };
      return entry.id
        ? request(`/api/entry/${encodeURIComponent(entry.id)}`, json("POST", payload))
        : request("/api/new", json("POST", { ...payload, source: entry.source || [] }));
    },
    async remove(id) { return request(`/api/entry/${encodeURIComponent(id)}`, { method: "DELETE" }); },
    async mark(id, level) { return request(`/api/mark/${encodeURIComponent(id)}`, json("POST", { level })); },
    async like(id, delta = 1) { return request(`/api/like/${encodeURIComponent(id)}`, json("POST", { delta })); },
    async createCategory(path, label) {
      return request("/api/category/create", json("POST", { path, label }));
    },
    async renameCategory(path, new_path, new_label) {
      return request("/api/category/rename", json("POST", { path, new_path, new_label }));
    },
    async deleteCategory(path) {
      return request("/api/category/delete", json("POST", { path }));
    }
  };
})();

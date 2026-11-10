(() => {
  "use strict";
  const $ = s => document.querySelector(s);
  const $$ = s => [...document.querySelectorAll(s)];
  const page = $("#page-content");
  const state = {
    categories: [], categoryMap: new Map(), stats: {},
    route: "/", category: "", query: "", sort: "relevance",
    selected: null, theme: localStorage.getItem("mcp-theme") || "light"
  };
  const esc = value => String(value ?? "").replace(/[&<>"']/g, c => ({ "&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;" }[c]));
  const tags = value => Array.isArray(value) ? value : [];
  const date = value => {
    if (!value) return "时间未知";
    const d = new Date(value);
    return Number.isNaN(+d) ? "时间未知" : new Intl.DateTimeFormat("zh-CN",{year:"numeric",month:"2-digit",day:"2-digit"}).format(d);
  };
  const stateName = value => ({current:"当前有效",archived:"已归档",obsolete:"已失效"}[value] || "未知状态");
  const stateClass = value => ["current","archived","obsolete"].includes(value) ? value : "current";
  const labelFor = path => state.categoryMap.get(path)?.label || path?.split(".").pop() || "未分类";
  const toast = message => {
    const node = document.createElement("div"); node.className = "toast"; node.textContent = message;
    $("#toast-region").append(node); setTimeout(() => node.remove(), 2800);
  };
  async function run(fn) {
    try { return await fn(); }
    catch (error) { toast(error.message || "操作失败"); throw error; }
  }

  /* 自定义确认框，替代 confirm() */
  function askConfirm(message, options = {}) {
    return new Promise(resolve => {
      const dlg = $("#confirm-dialog");
      $("#confirm-title").textContent = options.title || "确认操作";
      $("#confirm-eyebrow").textContent = options.eyebrow || "CONFIRM";
      $("#confirm-message").textContent = message;
      const ok = $("#confirm-ok");
      ok.textContent = options.okText || "确认";
      const done = value => { cleanup(); dlg.close(); resolve(value); };
      const onOk = () => done(true);
      const onCancel = () => done(false);
      const onBackdrop = e => { if (e.target === dlg) done(false); };
      function cleanup() {
        ok.removeEventListener("click", onOk);
        dlg.removeEventListener("click", onBackdrop);
        dlg.removeEventListener("close", onCancelClose);
      }
      function onCancelClose() { cleanup(); resolve(false); }
      ok.addEventListener("click", onOk);
      dlg.addEventListener("click", onBackdrop);
      dlg.addEventListener("close", onCancelClose);
      dlg.showModal();
    });
  }

  /* 自定义新建分类弹窗，替代两次 prompt() */
  function askCategory(defaultPath = "") {
    return new Promise(resolve => {
      const dlg = $("#category-dialog");
      const form = $("#category-form");
      const pathIn = $("#category-path");
      const labelIn = $("#category-label");
      const preview = $("#category-preview");

      pathIn.value = defaultPath;
      labelIn.value = defaultPath ? defaultPath.split(".").pop() : "";
      preview.textContent = "";

      const syncLabel = () => {
        const p = pathIn.value.trim();
        if (!labelIn.dataset.touched) labelIn.value = p ? p.split(".").pop() : "";
        preview.textContent = p ? `完整路径：${p}` : "";
      };
      const onPathInput = () => syncLabel();
      const onLabelInput = () => { labelIn.dataset.touched = "1"; };

      const done = value => {
        form.removeEventListener("submit", onSubmit);
        pathIn.removeEventListener("input", onPathInput);
        labelIn.removeEventListener("input", onLabelInput);
        delete labelIn.dataset.touched;
        dlg.close();
        resolve(value);
      };
      const onSubmit = e => {
        e.preventDefault();
        const path = pathIn.value.trim();
        const label = labelIn.value.trim();
        if (!path || !label) { toast("路径和名称不能为空"); return; }
        done({ path, label });
      };

      form.addEventListener("submit", onSubmit);
      pathIn.addEventListener("input", onPathInput);
      labelIn.addEventListener("input", onLabelInput);
      syncLabel();
      dlg.showModal();
      setTimeout(() => pathIn.focus(), 40);
    });
  }
  function categoryTree(cats) {
    const map = new Map(cats.map(c => [c.path, {...c, children:[]}]));
    const roots = [];
    cats.forEach(cat => {
      const node = map.get(cat.path);
      const parentPath = cat.path.includes(".") ? cat.path.slice(0, cat.path.lastIndexOf(".")) : "";
      if (map.has(parentPath)) map.get(parentPath).children.push(node);
      else roots.push(node);
    });
    const sort = list => list.sort((a,b) => a.label.localeCompare(b.label,"zh-CN")).forEach(n => sort(n.children));
    sort(roots); return roots;
  }
  function renderCategoryTree() {
    const draw = (nodes, depth=0) => nodes.map(n => `
      <a class="category-link depth-${Math.min(depth,3)}" href="/cat/${encodeURIComponent(n.path)}" data-category="${esc(n.path)}">
        <span class="category-bullet"></span><span class="category-name">${esc(n.label)}</span>
        <span class="category-count">${Number(n.count)||0}</span>
      </a>${draw(n.children,depth+1)}`).join("");
    const roots = categoryTree(state.categories);
    $("#category-tree").innerHTML = roots.length ? draw(roots) : `<div class="empty-side">暂无分类</div>`;
  }
  async function loadBase() {
    const [cats, stats] = await Promise.all([MCPApi.list(), MCPApi.stats()]);
    state.categories = cats; state.categoryMap = new Map(cats.map(c => [c.path,c]));
    state.stats = stats || {}; renderCategoryTree();
  }
  const stat = key => Number(state.stats[key]) || 0;
  const heading = (eyebrow,title,desc,action=true) => `
    <div class="page-heading"><div><div class="eyebrow">${eyebrow}</div><h1>${title}</h1><p>${desc}</p></div>
    ${action ? `<button class="button primary" id="new-entry"><span>＋</span> 记录新记忆</button>` : ""}</div>`;
  const card = (e, compact=false) => `
    <article class="memory-card ${compact?"compact":""}" data-open-entry="${esc(e.id)}">
      <div class="card-top"><span class="state-pill ${stateClass(e.state)}">${stateName(e.state)}</span>
        <span class="card-mark">${e.is_fav ? "★" : ""}${e.is_pick ? "✦" : ""}</span></div>
      <h3>${esc(e.title)}</h3>
      <p class="card-excerpt">${esc(e.snippet || e.effect || e.cause || "尚未添加摘要。")}</p>
      <div class="tag-row">${tags(e.tags).slice(0,3).map(t=>`<span class="tag">${esc(t)}</span>`).join("")}</div>
      <div class="card-meta"><span>${esc(labelFor(e.cat_path))}</span><span class="meta-dot"></span><span>${date(e.updated_at || e.created_at)}</span><span class="meta-spacer"></span><span>↗ ${Number(e.backlinks)||0}</span></div>
    </article>`;
  const empty = (title,desc,action=false) => `<div class="empty-state"><div class="empty-mark">∴</div><h3>${title}</h3><p>${desc}</p>${action?'<button class="button secondary" id="new-entry">＋ 记录第一条记忆</button>':""}</div>`;
  async function home() {
    const recent = await MCPApi.timeline({limit:6});
    const marked = await MCPApi.search({cat:"",q:"",sort:"marked",limit:4}).catch(()=>[]);
    page.innerHTML = `${heading("YOUR KNOWLEDGE, IN CONTEXT","让记忆成为可以检索的知识","记录事实、保留因果、连接上下文。知识不再散落，而是持续生长。")}
      <section class="stats-grid">
        <div class="stat-card"><span>知识条目</span><b>${stat("entries")}</b><small>已沉淀的记忆单元</small><i>01</i></div>
        <div class="stat-card"><span>知识分类</span><b>${stat("cats")}</b><small>层级化的主题索引</small><i>02</i></div>
        <div class="stat-card"><span>关联引用</span><b>${stat("links")}</b><small>条目之间的知识连接</small><i>03</i></div>
        <div class="stat-card"><span>标记记忆</span><b>${stat("marked")}</b><small>值得优先回顾的内容</small><i>04</i></div>
      </section>
      <div class="content-columns">
        <section class="primary-column">
          <div class="section-heading"><div><span class="eyebrow">RECENTLY CHANGED</span><h2>最近发生的变化</h2><p>按更新时间追踪知识的演进</p></div><a class="text-link" href="/timeline">完整时间线 ↗</a></div>
          <div class="memory-list">${recent.length?recent.map(e=>card(e,true)).join(""):empty("还没有记录","创建第一条记忆，开始构建自己的知识脉络。",true)}</div>
        </section>
        <aside class="secondary-column">
          <div class="section-heading"><div><span class="eyebrow">IMPORTANT NOTES</span><h2>值得回看的记忆</h2></div></div>
          <div class="memory-list">${marked.length?marked.map(e=>card(e,true)).join(""):`<div class="quiet-note">标记的条目会出现在这里。<br>在详情页可以收藏或设为精选。</div>`}</div>
          <div class="principle-card"><span class="principle-symbol">✳</span><b>记录上下文，而不只是答案</b><p>起因说明问题从何而来，结果保留决策的落点，正文保存完整过程。</p></div>
        </aside>
      </div>`;
  }
  async function timeline() {
    const items = await MCPApi.timeline({limit:300,cat:state.category || undefined});
    const grouped = new Map();
    items.forEach(e=>{const key=(e.updated_at||e.created_at||"").slice(0,7)||"未标注日期"; if(!grouped.has(key))grouped.set(key,[]);grouped.get(key).push(e);});
    page.innerHTML = `${heading("CHRONOLOGY","时间脉络","从时间维度回看知识的变化与决策的来龙去脉。")}
      <div class="toolbar"><span>${items.length} 条记录</span><span class="toolbar-spacer"></span><select id="timeline-category"><option value="">所有分类</option>${state.categories.map(c=>`<option value="${esc(c.path)}">${"　".repeat(Math.max(0,c.depth-1))}${esc(c.label)}</option>`).join("")}</select></div>
      <div class="timeline-groups">${[...grouped].map(([month,list])=>`<section class="timeline-month"><div class="month-label">${esc(month)}</div><div class="timeline-items">${list.map(e=>`<div class="timeline-item"><div class="timeline-date">${date(e.updated_at||e.created_at)}</div><div class="timeline-node"></div><div class="timeline-entry">${card(e,true)}</div></div>`).join("")}</div></section>`).join("") || empty("时间线还是空的","新增记忆后，它们会在这里按时间排列。",true)}</div>`;
    $("#timeline-category").value=state.category;
  }
  async function searchPage() {
    page.innerHTML = `${heading("RETRIEVE CONTEXT","检索你的记忆","不必记得它存在哪里。用关键词找回当时的上下文。",false)}
      <div class="search-page-box"><span>⌕</span><input id="page-search" value="${esc(state.query)}" placeholder="试试：端口、服务器、某个决策…"><select id="search-sort"><option value="relevance">相关度</option><option value="newest">最近更新</option><option value="marked">标记优先</option></select><button class="button primary" id="do-search">检索</button></div>
      <div class="toolbar"><span id="search-count">输入关键词以开始检索</span></div><div class="memory-grid" id="search-results"></div>`;
    $("#search-sort").value=state.sort;
    if(state.query) await doSearch();
  }
  async function doSearch() {
    state.query=$("#page-search")?.value.trim() ?? state.query;
    state.sort=$("#search-sort")?.value || state.sort;
    const out=$("#search-results"), count=$("#search-count");
    if(!state.query){out.innerHTML="";count.textContent="输入关键词以开始检索";return;}
    const results=await run(()=>MCPApi.search({q:state.query,sort:state.sort,limit:100}));
    count.textContent=`检索到 ${results.length} 条相关记忆`;
    out.innerHTML=results.length?results.map(e=>card(e)).join(""):empty("没有找到匹配内容","试试更短的关键词，或换一个描述方式。");
  }
  async function categoryPage(path) {
    state.category=path;
    const items=await run(()=>MCPApi.search({cat:path,sort:"newest",limit:100}));
    const cat=state.categoryMap.get(path);
    page.innerHTML=`${heading("KNOWLEDGE COLLECTION",esc(cat?.label||path.split(".").pop()),`分类路径 / ${esc(path)} · 包含该分类下的所有子级条目`)}
      <div class="toolbar"><span>${items.length} 条记录</span><span class="toolbar-spacer"></span><button class="button ghost small-button" id="clear-category">查看全部</button></div>
      <div class="memory-grid">${items.length?items.map(e=>card(e)).join(""):empty("这个分类还没有记录","可以在此分类下创建一条新的知识记录。",true)}</div>`;
  }
  function markdown(body) {
    if(window.marked?.parse) return window.marked.parse(body);
    return `<pre>${esc(body)}</pre>`;
  }
  function relationBlock(title,items) {
    if(!items?.length)return "";
    return `<section class="relation-section"><div class="section-heading"><div><span class="eyebrow">KNOWLEDGE LINKS</span><h3>${title}</h3></div><span class="relation-count">${items.length}</span></div><div class="relation-list">${items.map(x=>`<a class="relation-link" href="/entry/${encodeURIComponent(x.id)}" data-open-entry="${esc(x.id)}"><span class="relation-arrow">↗</span><span>${esc(x.title||`条目 #${x.id}`)}</span></a>`).join("")}</div></section>`;
  }
  async function entryPage(id) {
    const e=await run(()=>MCPApi.entry(id)); state.selected=e;
    page.innerHTML=`<div class="detail-back"><a href="/" class="back-link">← 知识总览</a><span>/</span><span>${esc(labelFor(e.cat_path))}</span></div>
      <article class="detail-page">
        <div class="detail-toolbar"><div class="detail-state"><span class="state-pill ${stateClass(e.state)}">${stateName(e.state)}</span>${e.state_reason?`<span class="state-reason">${esc(e.state_reason)}</span>`:""}</div>
          <div class="detail-actions"><button class="button ghost small-button ${e.is_fav?"marked":""}" id="fav-entry" title="收藏">★ ${e.is_fav?"已收藏":"收藏"}</button><button class="button ghost small-button ${e.is_pick?"marked":""}" id="pick-entry" title="精选">✦ ${e.is_pick?"已精选":"精选"}</button><button class="button ghost small-button ${e.likes?"marked":""}" id="like-entry" title="点赞">♡ ${e.likes}</button><button class="button secondary small-button" id="edit-entry">编辑</button><button class="button ghost small-button danger-text" id="delete-entry" title="删除记录">删除</button></div></div>
        <header class="article-header"><div class="eyebrow">MEMORY ENTRY · #${esc(e.id)}</div><h1>${esc(e.title)}</h1>
          <div class="detail-meta"><a href="/cat/${encodeURIComponent(e.cat_path)}" data-category="${esc(e.cat_path)}">${esc(labelFor(e.cat_path))}</a><span>·</span><span>创建 ${date(e.created_at)}</span><span>·</span><span>更新 ${date(e.updated_at)}</span><span>·</span><span>${e.hits} 次检索</span></div>
          <div class="detail-tags">${tags(e.tags).map(t=>`<span class="tag">${esc(t)}</span>`).join("")}</div>
        </header>
        ${e.cause?`<section class="insight-block cause-block"><div class="block-label"><span>01 / CONTEXT</span><b>起因与背景</b></div><p>${esc(e.cause)}</p></section>`:""}
        ${e.effect?`<section class="insight-block effect-block"><div class="block-label"><span>02 / OUTCOME</span><b>结果与结论</b></div><p>${esc(e.effect)}</p></section>`:""}
        <div class="article-body markdown-content">${e.body?markdown(e.body):`<p class="quiet-note">这条记录暂时没有补充正文。</p>`}</div>
        ${e.source.length?`<section class="source-section"><div class="eyebrow">ORIGINAL SOURCES</div><h3>来源文件</h3>${e.source.map(s=>`<div class="source-item">▤ ${esc(s)}</div>`).join("")}</section>`:""}
        ${relationBlock("引用了这些内容",e.links_out)}
        ${relationBlock("被这些内容引用",e.links_in)}
      </article>`;
  }
  function route() {
    const path=decodeURI(location.pathname||"/"); state.route=path; state.category="";
    const crumb=path.startsWith("/entry/")?"记忆详情":path.startsWith("/cat/")?"知识分类":path==="/timeline"?"时间脉络":path==="/search"?"检索记忆":"知识总览";
    $("#breadcrumbs").innerHTML=`<span>知识库</span><i>/</i><b>${crumb}</b>`;
    $$(".nav-item[data-route]").forEach(a=>a.classList.toggle("active",(path==="/"&&a.dataset.route==="home")||path.slice(1)===a.dataset.route));
    $("#sidebar").classList.remove("open");$("#overlay").classList.remove("show");
    const task=path==="/timeline"?timeline():path==="/search"?searchPage():path.startsWith("/cat/")?categoryPage(path.slice(5)):path.startsWith("/entry/")?entryPage(path.slice(7)):home();
    task.catch(err=>{page.innerHTML=`<div class="error-panel"><b>内容暂时无法载入</b><p>${esc(err.message||"请确认后端服务运行正常。")}</p><button class="button secondary" id="retry-load">重新载入</button></div>`;});
  }
  function go(path) {
    if(location.pathname!==path) history.pushState({}, "", path);
    route();
  }
  function openEditor(entry=null) {
    $("#entry-form").reset();
    $("#editor-title").textContent=entry?"编辑知识记录":"记录一段新记忆";
    $("#entry-id").value=entry?.id||"";
    $("#entry-title").value=entry?.title||"";
    $("#entry-category").value=entry?.cat_path||state.category||"";
    $("#entry-state").value=entry?.state||"current";
    $("#entry-cause").value=entry?.cause||"";
    $("#entry-effect").value=entry?.effect||"";
    $("#entry-body").value=entry?.body||"";
    $("#entry-tags").value=tags(entry?.tags).join(", ");
    $("#editor-dialog").showModal();
  }
  async function submitEntry(ev) {
    ev.preventDefault();
    const payload={
      id:$("#entry-id").value||undefined,title:$("#entry-title").value.trim(),
      cat_path:$("#entry-category").value.trim(),state:$("#entry-state").value,
      cause:$("#entry-cause").value.trim(),effect:$("#entry-effect").value.trim(),
      body:$("#entry-body").value,tags:$("#entry-tags").value.split(/[,，]/).map(x=>x.trim()).filter(Boolean)
    };
    if(!payload.title||!payload.cat_path){toast("标题和分类路径不能为空");return;}
    const btn=$("#save-entry");btn.disabled=true;btn.textContent="保存中…";
    try {
      const result=await run(()=>MCPApi.save(payload));$("#editor-dialog").close();toast("记录已保存");
      await loadBase();
      const id=payload.id||result?.id;
      if(id)go(`/entry/${id}`);else route();
    } finally {btn.disabled=false;btn.textContent="保存记录";}
  }
  async function quickSearch() {
    $("#search-dialog").showModal();$("#global-search").value="";$("#quick-results").innerHTML="";$("#global-search").focus();
  }
  let quickTimer;
  $("#global-search").addEventListener("input",()=>{
    clearTimeout(quickTimer);
    quickTimer=setTimeout(async()=>{
      const q=$("#global-search").value.trim();if(!q){$("#quick-results").innerHTML="";return;}
      try {
        const items=await MCPApi.search({q,limit:7,sort:"relevance"});
        $("#quick-results").innerHTML=items.map(e=>`<button class="quick-result" data-open-entry="${esc(e.id)}"><span class="quick-result-icon">↗</span><span><b>${esc(e.title)}</b><small>${esc(e.cat_path)} · ${esc(e.snippet||e.effect||"")}</small></span></button>`).join("")||`<div class="quick-empty">没有找到相关记忆</div>`;
      } catch(err) {$("#quick-results").innerHTML=`<div class="quick-empty">${esc(err.message)}</div>`;}
    },240);
  });
  document.addEventListener("click",async ev=>{
    const target=ev.target.closest("button,a,[data-open-entry]");if(!target)return;
    if(target.id==="new-entry")openEditor();
    if(target.id==="do-search")doSearch();
    if(target.id==="clear-category")go("/");
    if(target.id==="retry-load")route();
    if(target.id==="edit-entry")openEditor(state.selected);
    if(target.id==="delete-entry"&&state.selected){
      const ok = await askConfirm(`确定删除「${state.selected.title}」？此操作不可撤销。`, { title: "删除记录", eyebrow: "DELETE ENTRY", okText: "删除" });
      if(ok){
        await run(()=>MCPApi.remove(state.selected.id));toast("记录已删除");go("/");
      }
    }
    if(target.id==="fav-entry"&&state.selected){
      const e=state.selected; const next=!e.is_fav;
      const lv=(next&&e.is_pick)?3:e.is_pick?2:next?1:0;
      await run(()=>MCPApi.mark(e.id,lv));
      toast(next?"已加入收藏":"已取消收藏");
      entryPage(e.id);
    }
    if(target.id==="pick-entry"&&state.selected){
      const e=state.selected; const next=!e.is_pick;
      const lv=(e.is_fav&&next)?3:next?2:e.is_fav?1:0;
      await run(()=>MCPApi.mark(e.id,lv));
      toast(next?"已设为精选":"已取消精选");
      entryPage(e.id);
    }
    if(target.id==="like-entry"&&state.selected){
      await run(()=>MCPApi.like(state.selected.id,1));toast("已记录一次赞");entryPage(state.selected.id);
    }
    if(target.id==="search-trigger")quickSearch();
    if(target.id==="theme-toggle"){
      state.theme=state.theme==="dark"?"light":"dark";document.documentElement.dataset.theme=state.theme;localStorage.setItem("mcp-theme",state.theme);
    }
    if(target.id==="refresh-data"){await run(loadBase);toast("索引已刷新");route();}
    if(target.id==="add-category"){
      const input = await askCategory(state.category || "");
      if(!input) return;
      await run(()=>MCPApi.createCategory(input.path, input.label));
      await loadBase();
      toast("分类已创建");
    }
    const cat=target.closest("[data-category]");
    if(cat){ev.preventDefault();go(`/cat/${encodeURIComponent(cat.dataset.category)}`);}
    const open=target.closest("[data-open-entry]");
    if(open&&open.dataset.openEntry){ev.preventDefault();$("#search-dialog").open&&$("#search-dialog").close();go(`/entry/${encodeURIComponent(open.dataset.openEntry)}`);}
    if(target.id==="overlay"){$("#sidebar").classList.remove("open");$("#overlay").classList.remove("show");}
  });
  $("#entry-form").addEventListener("submit",submitEntry);
  $$("[data-close-dialog]").forEach(b=>b.addEventListener("click",()=>$("#editor-dialog").close()));
  $$("[data-close-mini]").forEach(b=>b.addEventListener("click",()=>b.closest("dialog").close()));
  $("#page-content").addEventListener("keydown",e=>{if(e.key==="Enter"&&e.target.id==="page-search")doSearch();});
  $("#page-content").addEventListener("change",e=>{
    if(e.target.id==="search-sort")doSearch();
    if(e.target.id==="timeline-category"){state.category=e.target.value;timeline();}
  });
  $("#menu-toggle").addEventListener("click",()=>{$("#sidebar").classList.add("open");$("#overlay").classList.add("show");});
  $("#overlay").addEventListener("click",()=>{$("#sidebar").classList.remove("open");$("#overlay").classList.remove("show");});
  $("#search-trigger").addEventListener("click",quickSearch);
  $("#global-search").addEventListener("keydown",e=>{if(e.key==="Enter"&&e.target.value.trim()){state.query=e.target.value.trim();$("#search-dialog").close();go("/search");}});
  $("#search-dialog").addEventListener("click",e=>{if(e.target===$("#search-dialog"))$("#search-dialog").close();});
  document.addEventListener("keydown",e=>{
    if((e.metaKey||e.ctrlKey)&&e.key.toLowerCase()==="k"){e.preventDefault();quickSearch();}
    if(e.key==="Escape"){$("#sidebar").classList.remove("open");$("#overlay").classList.remove("show");}
  });
  window.addEventListener("popstate",route);
  document.documentElement.dataset.theme=state.theme;
  loadBase().then(route).catch(err=>{page.innerHTML=`<div class="error-panel"><b>无法连接知识库</b><p>${esc(err.message)}</p></div>`;});
})();
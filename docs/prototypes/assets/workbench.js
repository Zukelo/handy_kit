/* HandyKit desktop prototype
 * Flow: fixed Home → tool cards / categorized navigation → unique tool tabs → close / switch.
 * State: open tabs, active tab, per-tool draft, favorites, recent tools, demo tasks.
 * Only workspace preferences persist. Tool input remains in memory for this session.
 * Direction: compact developer workbench; warm gray, forest green; 4px spacing; quiet motion.
 */
const tools = [
  { id: 'json', name: 'JSON 格式化', symbol: '{ }', category: '格式与校验', color: 'green', description: '让结构清晰，让数据易读', tags: '格式化 · 压缩 · 校验' },
  { id: 'xml', name: 'XML 格式化', symbol: '</>', category: '格式与校验', color: 'orange', description: '整理层级，检查文档结构', tags: '格式化 · 语法校验' },
  { id: 'timestamp', name: '时间戳转换', symbol: '◷', category: '时间与任务', color: 'blue', description: '在时间戳与日期之间自如转换', tags: '秒 / 毫秒 · 时区' },
  { id: 'cron', name: 'cron 表达式', symbol: '✳', category: '时间与任务', color: 'purple', description: '读懂每一次计划执行', tags: '表达式 · 执行预览' },
  { id: 'tasks', name: '定时任务', symbol: '☷', category: '时间与任务', color: 'rose', description: '把重复的小事交给计划', tags: '提醒 · 任务管理' },
];
const $ = (selector, root = document) => root.querySelector(selector);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const escape = value => String(value).replace(/[&<>"']/g, c => ({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'}[c]));
let saved = {};
try { saved = JSON.parse(localStorage.getItem('handykit.workspace.v1') || '{}'); } catch { /* storage is optional */ }
const valid = id => tools.some(tool => tool.id === id);
let tabs = ['home', ...new Set((Array.isArray(saved.tabs) ? saved.tabs : []).filter(valid))];
let active = tabs.includes(saved.active) ? saved.active : 'home';
let favorites = Array.isArray(saved.favorites) ? saved.favorites.filter(valid) : ['json', 'timestamp', 'cron'];
let recent = Array.isArray(saved.recent) ? saved.recent.filter(valid).slice(0, 4) : [];
const panels = new Map();
let demoTasks = [{ id: 1, name: '每日开发回顾', cron: '0 18 * * 1-5', enabled: true }, { id: 2, name: '站起来活动一下', cron: '0 * * * *', enabled: false }];
const icon = (tool, small = false) => `<span class="tool-icon ${tool.color} ${small ? 'small' : ''}" aria-hidden="true">${tool.symbol}</span>`;
const persist = () => { try { localStorage.setItem('handykit.workspace.v1', JSON.stringify({ tabs: tabs.filter(id => id !== 'home'), active, favorites, recent })); } catch { /* private mode */ } };
const brandUrl = new URL('./assets/brand.svg', document.currentScript?.src ?? location.href).href;
$('#app').innerHTML = `
  <header class="titlebar"><div class="brand"><img class="brand-logo" src="${brandUrl}" alt="" aria-hidden="true"><strong>小工坊 <span>HandyKit</span></strong><span class="version">PROTOTYPE / 0.2</span></div><div class="title-right"><span class="local-dot"></span> 本地工作空间 <span class="divider">/</span><span>开发者的随身工具箱</span><label class="appearance-control"><span>外观</span><select data-appearance aria-label="外观主题"><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></select></label></div></header>
  <div class="workbench"><aside class="sidebar"><div class="sidebar-top"><span class="eyebrow">WORKSPACE</span><button class="icon-button" id="collapse-nav" aria-label="收起导航" title="收起导航">◂</button></div>
  <button class="nav-home" data-open="home"><span aria-hidden="true">⌂</span><span>工作台首页</span><span class="home-key">H</span></button>
  <div class="nav-divider"></div><label class="search-box"><span aria-hidden="true">⌕</span><input id="tool-search" type="search" placeholder="查找工具…" aria-label="查找工具"><kbd>/</kbd></label>
  <nav aria-label="工具分类">${['格式与校验', '时间与任务'].map((category, i) => `<section class="nav-group"><button class="group-toggle" aria-expanded="true" aria-controls="group-${i}"><span>${category}</span><span class="chevron">⌄</span></button><div id="group-${i}" class="group-items">${tools.filter(tool => tool.category === category).map(tool => `<button class="nav-item" data-open="${tool.id}" data-search="${tool.name.toLowerCase()}">${icon(tool, true)}<span>${tool.name}</span></button>`).join('')}</div></section>`).join('')}<p id="no-results" hidden>没有找到匹配的工具</p></nav>
  <div class="sidebar-bottom"><div class="privacy-icon" aria-hidden="true">◇</div><strong>灵感在这里，数据也在这里</strong><p>工具数据在本机处理<br>专注手边的每一件小事。</p><span class="offline-badge"><i></i> 无需云端服务</span></div></aside>
  <div class="workspace"><div class="tabbar"><button class="reopen-nav icon-button" id="reopen-nav" aria-label="展开导航" title="展开导航">☰</button><div class="tabs" role="tablist" aria-label="已打开的工具"></div><button id="tab-menu" class="icon-button tab-menu" aria-label="标签页操作" title="标签页操作">···</button><div class="tab-dropdown" hidden><button id="close-others">关闭其他工具标签</button><button id="close-all">关闭全部工具标签</button></div></div><main id="content"></main></div></div>
  <footer><span><i class="status-dot"></i> 就绪 <span class="footer-separator">│</span><span id="footer-tool">工作台首页</span></span><span>GUI 交互原型 <span class="footer-separator">│</span> 数据仅在本机处理</span></footer>
  <div id="toast" role="status" aria-live="polite"></div>
  <dialog id="task-dialog"><form id="task-form"><div class="dialog-heading"><div><span class="eyebrow">NEW TASK</span><h2>新建定时任务</h2></div><button type="button" class="icon-button" id="cancel-task" aria-label="关闭">×</button></div><p class="muted">原型演示：保存到当前会话，不会实际执行。</p><label>任务名称<input name="taskName" maxlength="60" required placeholder="例如：每日开发回顾"></label><label>cron 表达式<input name="taskCron" required value="0 18 * * 1-5"></label><p id="task-error" class="error" role="alert"></p><div class="dialog-actions"><button type="button" class="button" id="cancel-task-bottom">取消</button><button type="submit" class="button primary">创建任务</button></div></form></dialog>`;
let toastTimer;
function toast(message) { $('#toast').textContent = message; $('#toast').classList.add('show'); clearTimeout(toastTimer); toastTimer = setTimeout(() => $('#toast').classList.remove('show'), 2600); }
function renderTabs(focus = false) {
  $('.tabs').innerHTML = tabs.map(id => { const tool = tools.find(t => t.id === id); const name = tool?.name || '首页'; return `<div class="tab ${active === id ? 'active' : ''}"><button role="tab" id="tab-${id}" aria-selected="${active === id}" aria-controls="panel-${id}" tabindex="${active === id ? 0 : -1}" data-tab="${id}">${tool ? icon(tool, true) : '<span class="tab-home" aria-hidden="true">⌂</span>'}<span>${name}</span></button>${id !== 'home' ? `<button class="tab-close" data-close="${id}" aria-label="关闭${name}" title="关闭">×</button>` : '<span class="pin" title="固定首页" aria-hidden="true">·</span>'}</div>`; }).join('');
  $$('[data-open]').forEach(button => button.classList.toggle('selected', button.dataset.open === active));
  $('#footer-tool').textContent = tools.find(tool => tool.id === active)?.name || '工作台首页';
  $(`#tab-${active}`)?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
  if (focus) $(`#tab-${active}`).focus();
}
function openTool(id, focus = false) {
  if (id !== 'home' && !valid(id)) return;
  if (!tabs.includes(id)) tabs.push(id);
  active = id;
  if (id !== 'home') recent = [id, ...recent.filter(item => item !== id)].slice(0, 4);
  if (id === 'home') renderHome(); else if (!panels.has(id)) createToolPanel(id);
  panels.forEach((panel, key) => { panel.hidden = key !== active; });
  renderTabs(focus); persist();
}
function closeTab(id) {
  if (id === 'home') return;
  const index = tabs.indexOf(id); tabs = tabs.filter(item => item !== id);
  // Preserve drafts in memory when a tab closes; reopening restores its content.
  openTool(active === id ? tabs[Math.max(0, index - 1)] : active, true);
}
function card(tool) { return `<article class="tool-card">${icon(tool)}<button class="favorite ${favorites.includes(tool.id) ? 'is-favorite' : ''}" data-favorite="${tool.id}" aria-label="${favorites.includes(tool.id) ? '取消收藏' : '收藏'}${tool.name}" aria-pressed="${favorites.includes(tool.id)}" title="${favorites.includes(tool.id) ? '取消收藏' : '收藏'}">${favorites.includes(tool.id) ? '★' : '☆'}</button><button class="card-link" data-open="${tool.id}"><h3>${tool.name}<span aria-hidden="true">↗</span></h3><p>${tool.description}</p><span class="card-tags">${tool.tags}</span></button></article>`; }
function renderHome(filter = 'all') {
  let panel = panels.get('home'); if (!panel) { panel = document.createElement('section'); panel.id = 'panel-home'; panel.className = 'home-panel'; panel.setAttribute('role', 'tabpanel'); panel.setAttribute('aria-labelledby', 'tab-home'); $('#content').append(panel); panels.set('home', panel); }
  const displayed = filter === 'favorites' ? tools.filter(tool => favorites.includes(tool.id)) : tools;
  panel.innerHTML = `<div class="page-breadcrumb">工作空间 <span>/</span> 首页</div><section class="welcome"><div><span class="eyebrow">A LITTLE HELP, EVERY DAY</span><h1>顺手的工具，<span>专注的工作。</span></h1><p>从一次格式化，到下一次计划。小工坊，接住开发中的小事。</p></div><div class="welcome-note"><span class="note-symbol" aria-hidden="true">{<i>✦</i>}</span><span>少一点切换，多一点心流</span></div></section>
  <section class="quick-section"><div class="section-heading"><h2>常用工具 <span>QUICK ACCESS</span></h2><span class="subtle">点击即在新标签页打开</span></div><div class="quick-grid">${favorites.length ? tools.filter(tool => favorites.includes(tool.id)).map(tool => `<button class="quick-item" data-open="${tool.id}">${icon(tool, true)}<span>${tool.name}</span><span class="quick-arrow" aria-hidden="true">↗</span></button>`).join('') : '<p class="muted">点击工具卡片上的星标，将它加入常用工具。</p>'}</div></section>
  <section><div class="section-heading"><h2>工具箱 <span>${String(tools.length).padStart(2, '0')} TOOLS</span></h2><div class="segment" aria-label="工具筛选"><button data-filter="all" class="${filter === 'all' ? 'active' : ''}" aria-pressed="${filter === 'all'}">全部工具</button><button data-filter="favorites" class="${filter === 'favorites' ? 'active' : ''}" aria-pressed="${filter === 'favorites'}">我的收藏</button></div></div><div class="tool-grid">${displayed.map(card).join('') || '<p class="muted">还没有收藏的工具。</p>'}</div></section>
  <section class="recent-section"><div class="section-heading"><h2>最近使用 <span>RECENT</span></h2>${recent.length ? '<button class="text-button" id="clear-recent">清空记录</button>' : ''}</div><div class="recent-list">${recent.length ? recent.map(id => { const tool = tools.find(t => t.id === id); return `<button data-open="${id}">${icon(tool, true)}<span>${tool.name}</span><span class="subtle">${tool.category}</span><span aria-hidden="true">→</span></button>`; }).join('') : '<div class="empty-recent"><span aria-hidden="true">↶</span> 从上方选择一个工具，开始今天的工作。</div>'}</div></section>
  <div class="home-footnote"><span>为日常开发，留一个顺手的角落。</span><span>HANDYKIT / 小工坊</span></div>`;
}
function panelHeader(tool, description, badge = '本地处理') { return `<div class="page-breadcrumb">工具箱 <span>/</span> ${tool.category} <span>/</span> ${tool.name}</div><div class="tool-heading"><div>${icon(tool)}<div><h1>${tool.name}</h1><p>${description}</p></div></div><span class="pill">${badge}</span></div>`; }
function createToolPanel(id) {
  const tool = tools.find(t => t.id === id); const panel = document.createElement('section'); panel.id = `panel-${id}`; panel.className = 'tool-panel'; panel.setAttribute('role', 'tabpanel'); panel.setAttribute('aria-labelledby', `tab-${id}`); panels.set(id, panel); $('#content').append(panel);
  if (id === 'json' || id === 'xml') {
    const sample = id === 'json' ? '{"project":"HandyKit","name":"小工坊","tools":["JSON","XML"],"local":true}' : '<workshop name="小工坊"><tool id="json">JSON 格式化</tool><tool id="xml">XML 格式化</tool></workshop>';
    panel.innerHTML = panelHeader(tool, tool.description) + `<div class="tool-toolbar"><div><button class="button primary" data-action="format">格式化</button>${id === 'json' ? '<button class="button" data-action="compact">压缩</button>' : ''}<button class="button" data-action="validate">校验</button></div><div><button class="text-button" data-action="sample">载入示例</button><button class="text-button" data-action="clear">清空</button></div></div><div class="editor-pair"><section class="editor-box"><div class="editor-heading"><label for="input-${id}">输入内容</label><span class="input-count">0 字符</span></div><textarea id="input-${id}" class="code-input" spellcheck="false" placeholder="在这里粘贴 ${id.toUpperCase()} 内容…">${escape(sample)}</textarea><div class="editor-footer">${id.toUpperCase()} <span>UTF-8</span></div></section><section class="editor-box"><div class="editor-heading"><label for="output-${id}">处理结果</label><button class="text-button" data-action="copy">复制结果</button></div><textarea id="output-${id}" class="code-output" spellcheck="false" readonly placeholder="处理结果将在这里显示"></textarea><div class="editor-footer">只读 <span>本机计算</span></div></section></div><p class="feedback" role="status" aria-live="polite">准备好了，试着整理一段数据吧。</p><p class="tip">${id === 'json' ? '原型使用浏览器 JSON 解析；超大整数可能丢失精度，正式版本需无损处理。' : 'XML 格式化为结构展示原型，不支持 DTD；包含混合文本或 xml:space 时拒绝格式化以保留语义。'}</p>`;
    const input = $('.code-input', panel), output = $('.code-output', panel), feedback = $('.feedback', panel);
    const count = () => $('.input-count', panel).textContent = `${input.value.length.toLocaleString()} 字符`; count(); input.addEventListener('input', count);
    panel.addEventListener('click', async event => {
      const action = event.target.closest('[data-action]')?.dataset.action; if (!action) return;
      feedback.classList.remove('error');
      if (action === 'sample') { input.value = sample; output.value = ''; count(); feedback.textContent = '已载入示例'; return; }
      if (action === 'clear') { input.value = output.value = ''; count(); feedback.textContent = '已清空'; input.focus(); return; }
      if (action === 'copy') { if (!output.value) { toast('请先生成处理结果'); return; } try { await navigator.clipboard.writeText(output.value); toast('结果已复制'); } catch { output.focus(); output.select(); toast('请按 Ctrl / ⌘ + C 复制选中的结果'); } return; }
      try {
        if (!input.value.trim()) throw new Error('请输入需要处理的内容');
        if (input.value.length > 1_000_000) throw new Error('原型最多处理 100 万字符');
        if (id === 'json') { const parsed = JSON.parse(input.value); output.value = action === 'validate' ? '' : JSON.stringify(parsed, null, action === 'compact' ? 0 : 2); }
        else {
          if (/<!DOCTYPE|<!ENTITY/i.test(input.value)) throw new Error('原型不支持 DTD 或实体声明');
          const doc = new DOMParser().parseFromString(input.value, 'application/xml');
          if (doc.querySelector('parsererror')) throw new Error('XML 结构无效，请检查标签闭合与属性引号');
          if (action === 'validate') output.value = '';
          else {
            if ($$('*', doc).some(el => el.hasAttribute('xml:space') || (el.children.length && [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim())))) throw new Error('包含混合文本或 xml:space，仅支持校验，避免改变文本语义');
            const render = (node, depth = 0) => {
              if (node.nodeType !== 1) return new XMLSerializer().serializeToString(node);
              if (!node.children.length) return '  '.repeat(depth) + new XMLSerializer().serializeToString(node);
              const clone = node.cloneNode(false); const serialized = new XMLSerializer().serializeToString(clone); const opening = serialized.replace(/\s*\/>$/, '>');
              return '  '.repeat(depth) + opening + '\n' + [...node.childNodes].filter(n => n.nodeType !== 3 || n.textContent.trim()).map(n => render(n, depth + 1)).join('\n') + '\n' + '  '.repeat(depth) + `</${node.nodeName}>`;
            }; output.value = render(doc.documentElement);
          }
        }
        feedback.textContent = action === 'validate' ? '✓ 校验通过，结构有效。' : '✓ 处理完成。输入内容已保留。';
      } catch (error) { output.value = ''; feedback.textContent = error.message; feedback.classList.add('error'); }
    });
  } else if (id === 'timestamp') {
    panel.innerHTML = panelHeader(tool, tool.description) + `<div class="info-strip"><span class="live-dot"></span> 当前 Unix 时间戳 <strong id="current-timestamp"></strong><button class="text-button" id="use-now">使用当前时间</button></div><div class="conversion-grid"><section class="form-card"><span class="eyebrow">TIMESTAMP → DATE</span><h2>时间戳转日期</h2><label>时间戳<input id="timestamp-value" inputmode="numeric" value="1795132800"></label><label>单位<select id="timestamp-unit"><option value="seconds">秒（s）</option><option value="milliseconds">毫秒（ms）</option></select></label><label>显示时区<select id="timestamp-zone"><option value="Asia/Shanghai">Asia/Shanghai · 北京时间</option><option value="UTC">UTC · 协调世界时</option></select></label><button class="button primary" id="convert-timestamp">转换为日期</button><output id="date-result" class="result-box">等待转换</output></section><section class="form-card"><span class="eyebrow">DATE → TIMESTAMP</span><h2>日期转时间戳</h2><label>日期与时间<input id="date-value" type="datetime-local" step="1" value="2026-11-20T09:00:00"></label><label>输入时区<select id="date-zone"><option value="+08:00">北京时间（UTC+08:00）</option><option value="Z">UTC（UTC+00:00）</option></select></label><p class="field-note">输出同时提供秒与毫秒，不自动猜测输入单位。</p><button class="button primary" id="convert-date">转换为时间戳</button><output id="timestamp-result" class="result-box">等待转换</output></section></div><p class="feedback" role="status" aria-live="polite"></p>`;
    const updateClock = () => { if (panel.isConnected) $('#current-timestamp', panel).textContent = String(Math.floor(Date.now() / 1000)); }; updateClock();
    // Session sample; refresh current time explicitly rather than a background timer.
    $('#use-now', panel).onclick = () => { updateClock(); $('#timestamp-value', panel).value = String($('#timestamp-unit', panel).value === 'seconds' ? Math.floor(Date.now() / 1000) : Date.now()); $('#convert-timestamp', panel).click(); };
    $('#convert-timestamp', panel).onclick = () => { const raw = $('#timestamp-value', panel).value.trim(); const result = $('#date-result', panel); try { if (!/^-?\d+$/.test(raw) || !Number.isSafeInteger(Number(raw))) throw new Error('请输入安全范围内的整数时间戳'); const date = new Date(Number(raw) * ($('#timestamp-unit', panel).value === 'seconds' ? 1000 : 1)); if (isNaN(date.getTime())) throw new Error('时间戳超出有效日期范围'); result.textContent = new Intl.DateTimeFormat('zh-CN', { timeZone: $('#timestamp-zone', panel).value, year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).format(date); } catch (error) { result.textContent = error.message; } };
    $('#convert-date', panel).onclick = () => { const input = $('#date-value', panel).value; const result = $('#timestamp-result', panel); const date = new Date(input + $('#date-zone', panel).value); result.textContent = !input || isNaN(date.getTime()) ? '请选择有效的日期与时间' : `${Math.floor(date.getTime() / 1000)} 秒\n${date.getTime()} 毫秒`; };
  } else if (id === 'cron') {
    panel.innerHTML = panelHeader(tool, tool.description, '交互演示') + `<section class="cron-card"><label for="cron-value">cron 表达式 <span class="muted">标准五字段</span></label><div class="cron-input-row"><input id="cron-value" value="0 9 * * 1-5" spellcheck="false"><button class="button primary" id="parse-cron">解析表达式</button></div><div class="cron-fields">${['分钟', '小时', '日', '月', '星期'].map((name, i) => `<div><strong data-cron-field="${i}">${['0', '9', '*', '*', '1-5'][i]}</strong><span>${name}</span></div>`).join('')}</div><div class="cron-presets"><span class="muted">试一试</span><button data-cron="0 9 * * 1-5">工作日 09:00</button><button data-cron="*/5 * * * *">每 5 分钟</button><button data-cron="0 0 * * *">每天零点</button></div></section><div class="cron-summary"><span class="eyebrow">EXPLANATION</span><h2 id="cron-explanation">每周一至周五，上午 09:00 执行</h2><p>Asia/Shanghai · 北京时间</p></div><section class="form-card"><h2>接下来的执行时间</h2><p class="muted">原型仅提供上述三种预设的真实时间预览；完整 cron 解析器将在 Rust 阶段接入。</p><ol id="cron-next" class="next-runs"></ol><p id="cron-error" class="error" role="alert"></p></section>`;
    const parse = () => { const value = $('#cron-value', panel).value.trim().replace(/\s+/g, ' '); const parts = value.split(' '); const descriptions = { '0 9 * * 1-5': '每周一至周五，上午 09:00 执行', '*/5 * * * *': '每 5 分钟执行一次', '0 0 * * *': '每天 00:00 执行' }; $$('[data-cron-field]', panel).forEach((el, i) => el.textContent = parts[i] || '—'); $('#cron-next', panel).innerHTML = ''; $('#cron-error', panel).textContent = ''; if (!descriptions[value]) { $('#cron-explanation', panel).textContent = '暂时无法预览此表达式'; $('#cron-error', panel).textContent = parts.length !== 5 ? '标准 cron 应包含五个字段：分钟、小时、日、月、星期。' : '当前原型仅支持下方三种预设，其他表达式尚未接入解析。'; return; } $('#cron-explanation', panel).textContent = descriptions[value]; const dates = []; let cursor = new Date(Math.floor(Date.now() / 60000) * 60000 + 60000); for (let i = 0; i < 60 * 24 * 9 && dates.length < 5; i++, cursor = new Date(cursor.getTime() + 60000)) { const beijing = new Date(cursor.getTime() + 8 * 3600000); const hour = beijing.getUTCHours(), minute = beijing.getUTCMinutes(), day = beijing.getUTCDay(); if (value === '*/5 * * * *' ? minute % 5 === 0 : value === '0 0 * * *' ? hour === 0 && minute === 0 : hour === 9 && minute === 0 && day >= 1 && day <= 5) dates.push(new Intl.DateTimeFormat('zh-CN', { timeZone: 'Asia/Shanghai', year: 'numeric', month: '2-digit', day: '2-digit', weekday: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).format(cursor)); } $('#cron-next', panel).innerHTML = dates.map(date => `<li>${escape(date)}<span>UTC+08:00</span></li>`).join(''); };
    $('#parse-cron', panel).onclick = parse; $$('[data-cron]', panel).forEach(button => button.onclick = () => { $('#cron-value', panel).value = button.dataset.cron; parse(); }); parse();
  } else {
    panel.innerHTML = panelHeader(tool, tool.description, '会话演示') + `<div class="demo-notice">此页演示任务管理流程。任务仅保留在当前会话，不执行后台调度或系统通知。</div><div class="section-heading task-heading"><h2>我的任务 <span id="task-count"></span></h2><button class="button primary" id="add-task">＋ 新建任务</button></div><div class="task-table-wrap"><table class="task-table"><thead><tr><th>任务名称</th><th>执行计划</th><th>时区</th><th>状态</th><th>操作</th></tr></thead><tbody></tbody></table></div><p class="tip">演示任务为示例数据；实际调度、执行记录及持久化将在后续阶段接入。</p>`;
    $('#add-task', panel).onclick = () => { $('#task-form').reset(); $('#task-error').textContent = ''; $('#task-dialog').showModal(); };
    renderTasks();
  }
}
function renderTasks() {
  const panel = panels.get('tasks'); if (!panel) return;
  $('#task-count', panel).textContent = `${demoTasks.length} TASKS`;
  $('tbody', panel).innerHTML = demoTasks.length ? demoTasks.map(task => `<tr><td><strong>${escape(task.name)}</strong><span class="table-sub">本地提醒 · 演示</span></td><td><code>${escape(task.cron)}</code></td><td>Asia/Shanghai</td><td><button class="toggle ${task.enabled ? 'on' : ''}" role="switch" aria-checked="${task.enabled}" aria-label="启停${escape(task.name)}" data-toggle-task="${task.id}"><i></i></button><span class="task-state">${task.enabled ? '已启用' : '已暂停'}</span></td><td><button class="text-button danger" data-delete-task="${task.id}">删除</button></td></tr>`).join('') : '<tr><td colspan="5" class="empty-cell">还没有任务，点击「新建任务」添加一个。</td></tr>';
}
$('#task-form').onsubmit = event => { event.preventDefault(); const form = new FormData(event.target); const name = String(form.get('taskName')).trim(), cron = String(form.get('taskCron')).trim(); if (!name || !cron) { $('#task-error').textContent = '请填写任务名称和表达式'; return; } if (cron.split(/\s+/).length !== 5 || !/^[\d\s*,/\-]+$/.test(cron)) { $('#task-error').textContent = '请使用五字段 cron 表达式；完整语义校验尚未接入。'; return; } demoTasks.push({ id: Date.now(), name, cron, enabled: false }); renderTasks(); $('#task-dialog').close(); toast('演示任务已创建，默认暂停'); };
$('#cancel-task').onclick = $('#cancel-task-bottom').onclick = () => $('#task-dialog').close();
document.addEventListener('click', event => {
  const target = event.target.closest('button'); if (!target) return;
  if (target.dataset.open) openTool(target.dataset.open);
  if (target.dataset.tab) openTool(target.dataset.tab);
  if (target.dataset.close) closeTab(target.dataset.close);
  if (target.dataset.favorite) { const id = target.dataset.favorite; favorites = favorites.includes(id) ? favorites.filter(item => item !== id) : [...favorites, id]; persist(); const filter = $('[data-filter="favorites"]', panels.get('home'))?.classList.contains('active') ? 'favorites' : 'all'; renderHome(filter); }
  if (target.dataset.filter) renderHome(target.dataset.filter);
  if (target.id === 'clear-recent') { recent = []; persist(); renderHome(); }
  if (target.classList.contains('group-toggle')) { const expanded = target.getAttribute('aria-expanded') === 'true'; target.setAttribute('aria-expanded', String(!expanded)); document.getElementById(target.getAttribute('aria-controls')).hidden = expanded; }
  if (target.dataset.toggleTask) { const task = demoTasks.find(item => item.id === Number(target.dataset.toggleTask)); task.enabled = !task.enabled; renderTasks(); toast(task.enabled ? '演示状态已启用，不会实际执行' : '演示任务已暂停'); }
  if (target.dataset.deleteTask) { const task = demoTasks.find(item => item.id === Number(target.dataset.deleteTask)); if (window.confirm(`删除演示任务「${task.name}」？`)) { demoTasks = demoTasks.filter(item => item !== task); renderTasks(); toast('演示任务已删除'); } }
});
$('.tabs').addEventListener('keydown', event => { const tab = event.target.closest('[role="tab"]'); if (!tab) return; const index = tabs.indexOf(tab.dataset.tab); let next; if (event.key === 'ArrowRight') next = tabs[(index + 1) % tabs.length]; if (event.key === 'ArrowLeft') next = tabs[(index - 1 + tabs.length) % tabs.length]; if (event.key === 'Home') next = tabs[0]; if (event.key === 'End') next = tabs[tabs.length - 1]; if (next) { event.preventDefault(); openTool(next, true); } if (event.key === 'Delete' && active !== 'home') { event.preventDefault(); closeTab(active); } });
$('#tool-search').addEventListener('input', event => { const value = event.target.value.trim().toLowerCase(); let matches = 0; $$('.nav-item').forEach(button => { button.hidden = !button.dataset.search.includes(value); if (!button.hidden) matches++; }); $$('.nav-group').forEach(group => { group.hidden = !$$('.nav-item', group).some(button => !button.hidden); if (value) { $('.group-toggle', group).setAttribute('aria-expanded', 'true'); $('.group-items', group).hidden = false; } }); $('#no-results').hidden = matches > 0; });
$('#collapse-nav').onclick = () => { $('.workbench').classList.add('nav-collapsed'); $('#reopen-nav').focus(); };
$('#reopen-nav').onclick = () => { $('.workbench').classList.remove('nav-collapsed'); $('#collapse-nav').focus(); };
$('#tab-menu').onclick = () => { $('.tab-dropdown').hidden = !$('.tab-dropdown').hidden; $('#tab-menu').setAttribute('aria-expanded', String(!$('.tab-dropdown').hidden)); };
$('#close-all').onclick = () => { tabs = ['home']; openTool('home', true); $('.tab-dropdown').hidden = true; };
$('#close-others').onclick = () => { tabs = active === 'home' ? ['home'] : ['home', active]; openTool(active, true); $('.tab-dropdown').hidden = true; };
document.addEventListener('click', event => { if (!event.target.closest('.tab-dropdown') && !event.target.closest('#tab-menu')) $('.tab-dropdown').hidden = true; });
document.addEventListener('keydown', event => { const typing = event.target.closest('input, textarea, select, [contenteditable="true"]'); if (event.key === '/' && !typing && !$('#task-dialog').open) { event.preventDefault(); $('.workbench').classList.remove('nav-collapsed'); $('#tool-search').focus(); } if (event.key === 'Escape') $('.tab-dropdown').hidden = true; });
renderHome(); openTool(active);

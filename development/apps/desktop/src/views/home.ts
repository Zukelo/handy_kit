import { tools, findTool } from '../app/tools.ts';
import type { WorkspaceState } from '../app/workspace.ts';
import { toolIcon } from '../components/html.ts';

export function renderHome(state: WorkspaceState): string {
  const selected = state.homeFilter === 'favorites' ? tools.filter(tool => state.favorites.includes(tool.id)) : tools;
  return `
    <div class="page-breadcrumb">工作空间 <span>/</span> 首页</div>
    <section class="welcome"><div><span class="eyebrow">A LITTLE HELP, EVERY DAY</span><h1>顺手的工具，<span>专注的工作。</span></h1><p>从一次格式化，到下一次计划。小工坊，接住开发中的小事。</p></div><div class="welcome-note"><span class="note-symbol" aria-hidden="true">{<i>✦</i>}</span><span>少一点切换，多一点心流</span></div></section>
    <section class="quick-section" aria-labelledby="quick-title">
      <div class="section-heading"><h2 id="quick-title">常用工具 <span>QUICK ACCESS</span></h2><span class="subtle">点击即在新标签页打开</span></div>
      <div class="quick-grid">${state.favorites.length ? state.favorites.map(id => findTool(id)!).map(tool => `<button class="quick-item" data-open="${tool.id}">${toolIcon(tool, true)}<span>${tool.name}</span><span class="quick-arrow" aria-hidden="true">↗</span></button>`).join('') : '<div class="home-empty">还没有常用工具，点击下方卡片的星标添加收藏。</div>'}</div>
    </section>
    <section aria-labelledby="toolbox-title">
      <div class="section-heading"><h2 id="toolbox-title">工具箱 <span>${String(tools.length).padStart(2, '0')} TOOLS</span></h2><div class="segment" aria-label="工具筛选"><button data-filter="all" class="${state.homeFilter === 'all' ? 'active' : ''}" aria-pressed="${state.homeFilter === 'all'}">全部工具</button><button data-filter="favorites" class="${state.homeFilter === 'favorites' ? 'active' : ''}" aria-pressed="${state.homeFilter === 'favorites'}">我的收藏</button></div></div>
      <div class="tool-grid">${selected.length ? selected.map(tool => {
        const favorite = state.favorites.includes(tool.id);
        return `<article class="tool-card">${toolIcon(tool)}<button class="favorite ${favorite ? 'is-favorite' : ''}" data-favorite="${tool.id}" aria-label="${favorite ? '取消收藏' : '收藏'}${tool.name}" aria-pressed="${favorite}" title="${favorite ? '取消收藏' : '收藏'}">${favorite ? '★' : '☆'}</button><button class="card-link" data-open="${tool.id}"><h3>${tool.name}<span aria-hidden="true">↗</span></h3><p>${tool.description}</p><span class="card-tags">${tool.tags}</span></button><span class="tool-status">待开发</span></article>`;
      }).join('') : '<div class="home-empty">还没有收藏的工具。切换到「全部工具」添加喜欢的工具。</div>'}</div>
    </section>
    <section class="recent-section" aria-labelledby="recent-title"><div class="section-heading"><h2 id="recent-title" tabindex="-1">最近打开 <span>RECENT</span></h2>${state.recent.length ? '<button class="text-button" data-action="clear-recent">清空记录</button>' : ''}</div><div class="recent-list">${state.recent.length ? state.recent.map(id => findTool(id)!).map(tool => `<button data-open="${tool.id}">${toolIcon(tool, true)}<span>${tool.name}</span><span class="subtle">${tool.category}</span><span aria-hidden="true">→</span></button>`).join('') : '<div class="empty-recent"><span aria-hidden="true">↶</span> 从上方选择一个工具，开始今天的工作。</div>'}</div></section>
    <div class="home-footnote"><span>为日常开发，留一个顺手的角落。</span><span>HANDYKIT / 小工坊</span></div>`;
}

import { categories, tools } from '../app/tools.ts';
import { toolIcon } from './html.ts';

export function renderShell(version: string): string {
  return `
    <header class="titlebar">
      <div class="brand"><span class="brand-mark" aria-hidden="true">h<span>k</span></span><strong>小工坊 <span>HandyKit</span></strong><span class="version">DEV / ${version}</span></div>
      <div class="title-right"><span class="local-dot"></span> 本地工作空间 <span class="divider">/</span><span>开发者的随身工具箱</span></div>
    </header>
    <div class="workbench">
      <aside class="sidebar" id="workspace-sidebar" aria-label="工具导航">
        <div class="sidebar-top"><span class="eyebrow">WORKSPACE</span><button class="icon-button" data-action="collapse-sidebar" aria-label="收起导航" title="收起导航">◂</button></div>
        <button class="nav-home" data-open="home"><span aria-hidden="true">⌂</span><span>工作台首页</span><span class="home-key" title="Alt + H">H</span></button>
        <div class="nav-divider"></div>
        <label class="search-box"><span aria-hidden="true">⌕</span><input id="tool-search" type="search" placeholder="查找工具…" aria-label="查找工具" autocomplete="off"><kbd>/</kbd></label>
        <nav aria-label="工具分类">
          ${categories.map((category, index) => `
            <section class="nav-group" data-category="${category}">
              <button class="group-toggle" data-group="${category}" aria-expanded="true" aria-controls="group-${index}"><span>${category}</span><span class="chevron" aria-hidden="true">⌄</span></button>
              <div id="group-${index}" class="group-items">
                ${tools.filter(tool => tool.category === category).map(tool => `<button class="nav-item" data-open="${tool.id}" title="${tool.name}">${toolIcon(tool, true)}<span>${tool.name}</span></button>`).join('')}
              </div>
            </section>`).join('')}
          <p id="no-results" role="status" hidden>没有找到匹配的工具</p>
        </nav>
        <div class="sidebar-bottom"><div class="privacy-icon" aria-hidden="true">◇</div><strong>灵感在这里，数据也在这里</strong><p>收藏与工作区保存在本机<br>专注手边的每一件小事。</p><span class="offline-badge"><i></i> 无需云端服务</span></div>
      </aside>
      <div class="workspace">
        <div class="tabbar">
          <button class="reopen-nav icon-button" data-action="expand-sidebar" aria-label="展开导航" aria-controls="workspace-sidebar" title="展开导航">☰</button>
          <div class="tabs" role="tablist" aria-label="已打开的工具"></div>
          <button id="tab-menu" class="icon-button tab-menu" data-action="toggle-tab-menu" aria-label="标签页操作" aria-expanded="false" aria-controls="tab-dropdown" title="标签页操作">···</button>
          <div class="tab-dropdown" id="tab-dropdown" hidden><button data-action="close-others">关闭其他工具标签</button><button data-action="close-all">关闭全部工具标签</button></div>
        </div>
        <main id="content" aria-label="工作区内容"></main>
      </div>
    </div>
    <footer><span><i class="status-dot"></i> 就绪 <span class="footer-separator">│</span><span id="footer-tool">工作台首页</span></span><span id="storage-status">工作区自动保存到本机</span></footer>
    <div id="toast" role="status" aria-live="polite" aria-atomic="true"></div>`;
}

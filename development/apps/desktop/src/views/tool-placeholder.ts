import type { Tool } from '../app/tools.ts';
import { toolIcon } from '../components/html.ts';

/** New tools will plug in here without changing workspace navigation or tab behavior. */
export function renderToolPlaceholder(tool: Tool): string {
  return `<div class="page-breadcrumb">工具箱 <span>/</span> ${tool.category} <span>/</span> ${tool.name}</div>
    <div class="tool-heading"><div>${toolIcon(tool)}<div><h1>${tool.name}</h1><p>${tool.description}</p></div></div><span class="pill">待开发</span></div>
    <section class="placeholder-card" aria-labelledby="placeholder-title-${tool.id}">
      <span class="placeholder-symbol" aria-hidden="true">${toolIcon(tool)}</span>
      <span class="eyebrow">COMING NEXT</span>
      <h2 id="placeholder-title-${tool.id}">工具就位，功能即将开启</h2>
      <p>这里是「${tool.name}」的工作区。<br>当前版本已完成应用框架与首页，该工具将在后续版本接入。</p>
      <div class="placeholder-tags">${tool.tags}</div>
      <div class="placeholder-actions"><button class="button primary" data-open="home">返回工作台</button><button class="button" data-placeholder-favorite="${tool.id}"></button></div>
    </section>`;
}

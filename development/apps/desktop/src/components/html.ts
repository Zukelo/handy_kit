import type { Tool } from '../app/tools.ts';
export function escapeHTML(value: string): string {
  return value.replace(
    /[&<>"']/g,
    (character) =>
      ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]!,
  );
}
export function toolIcon(tool: Tool, small = false): string {
  return `<span class="tool-icon ${tool.color} ${small ? 'small' : ''}" aria-hidden="true">${escapeHTML(tool.symbol)}</span>`;
}
export function requiredElement<T extends Element>(
  selector: string,
  root: ParentNode = document,
): T {
  const element = root.querySelector<T>(selector);
  if (!element) throw new Error(`Missing application element: ${selector}`);
  return element;
}

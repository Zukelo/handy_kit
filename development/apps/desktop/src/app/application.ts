import { closeViews, type ClosableView } from './lifecycle.ts';
import { PluginsView } from '../plugins/view.ts';
import { TasksView } from '../tasks/view.ts';
import {
  categories,
  findTool,
  isTabId,
  isToolId,
  matchesSearch,
  type Category,
  type TabId,
} from './tools.ts';
import {
  closeAllTabs,
  closeOtherTabs,
  closeTab,
  loadWorkspace,
  openTab,
  saveWorkspace,
  toggleFavorite,
  type StoragePort,
  type WorkspaceState,
} from './workspace.ts';
import { requiredElement, toolIcon } from '../components/html.ts';
import { renderShell } from '../components/shell.ts';
import { renderHome } from '../views/home.ts';
import { renderToolPlaceholder } from '../views/tool-placeholder.ts';

export class Application {
  private state: WorkspaceState;
  private transitionSequence = 0;
  private tasksView: TasksView | undefined;
  private pluginsView: PluginsView | undefined;
  private readonly root: HTMLElement;
  private readonly storage: StoragePort;
  private storageAvailable: boolean;
  private readonly panels = new Map<TabId, HTMLElement>();
  private readonly scrollPositions = new Map<TabId, number>();
  private toastTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(root: HTMLElement, storage: StoragePort, version: string) {
    this.root = root;
    this.storage = storage;
    const loaded = loadWorkspace(storage);
    this.state = loaded.state;
    this.storageAvailable = loaded.available;
    root.innerHTML = renderShell(version);
    // All restored tabs get matching panels, including currently inactive tabs.
    this.state.tabs.forEach((id) => this.ensurePanel(id));
    this.render();
    root.addEventListener('click', this.onClick);
    root.addEventListener('keydown', this.onKeyDown);
    this.element<HTMLInputElement>('#tool-search').addEventListener('input', () =>
      this.renderNavigation(),
    );
    document.addEventListener('pointerdown', (event) => {
      if (event.target instanceof Element && !event.target.closest('#tab-menu, #tab-dropdown'))
        this.setTabMenu(false);
    });
  }

  private element<T extends Element>(selector: string): T {
    return requiredElement<T>(selector, this.root);
  }
  private buttons(selector: string): HTMLButtonElement[] {
    return [...this.root.querySelectorAll<HTMLButtonElement>(selector)];
  }
  private notify(message: string): void {
    const toast = this.element<HTMLElement>('#toast');
    toast.textContent = message;
    toast.classList.add('show');
    clearTimeout(this.toastTimer);
    this.toastTimer = setTimeout(() => toast.classList.remove('show'), 2600);
  }
  private persist(): void {
    this.storageAvailable = saveWorkspace(this.storage, this.state);
    this.element<HTMLElement>('#storage-status').textContent = this.storageAvailable
      ? '工作区自动保存到本机'
      : '当前会话模式 · 工作区无法保存';
  }
  private update(next: WorkspaceState, focusSelector?: string): void {
    const sequence = ++this.transitionSequence;
    const closing: ClosableView[] = [];
    if (!next.tabs.includes('tasks') && this.tasksView) closing.push(this.tasksView);
    if (!next.tabs.includes('plugins') && this.pluginsView) closing.push(this.pluginsView);
    void closeViews(
      closing,
      () => sequence === this.transitionSequence,
      () => {
        if (!next.tabs.includes('tasks')) this.tasksView = undefined;
        if (!next.tabs.includes('plugins')) this.pluginsView = undefined;
        this.commitUpdate(next, focusSelector);
      },
    ).catch((error) => this.notify(String(error)));
  }
  private commitUpdate(next: WorkspaceState, focusSelector?: string): void {
    const content = this.element<HTMLElement>('#content');
    this.scrollPositions.set(this.state.active, content.scrollTop);
    this.state = next;
    this.state.tabs.forEach((id) => this.ensurePanel(id));
    // Discard closed tool views. Actual tools can add a draft lifecycle when implemented.
    for (const [id, panel] of this.panels) {
      if (!this.state.tabs.includes(id)) {
        panel.remove();
        this.panels.delete(id);
        this.scrollPositions.delete(id);
      }
    }
    this.render();
    content.scrollTop = this.scrollPositions.get(this.state.active) ?? 0;
    this.persist();
    if (focusSelector)
      this.root.querySelector<HTMLElement>(focusSelector)?.focus({ preventScroll: true });
  }
  private ensurePanel(id: TabId): void {
    if (this.panels.has(id)) return;
    const panel = document.createElement('section');
    panel.id = `panel-${id}`;
    panel.className = id === 'home' ? 'home-panel' : 'tool-panel';
    panel.setAttribute('role', 'tabpanel');
    panel.setAttribute('aria-labelledby', `tab-${id}`);
    panel.tabIndex = 0;
    if (id !== 'home' && id !== 'tasks' && id !== 'plugins')
      panel.innerHTML = renderToolPlaceholder(findTool(id)!);
    this.element<HTMLElement>('#content').append(panel);
    this.panels.set(id, panel);
    if (id === 'tasks') this.tasksView = new TasksView(panel);
    if (id === 'plugins') this.pluginsView = new PluginsView(panel);
  }
  private render(): void {
    this.panels.get('home')!.innerHTML = renderHome(this.state);
    this.panels.forEach((panel, id) => {
      panel.hidden = id !== this.state.active;
    });
    this.element<HTMLElement>('.tabs').innerHTML = this.state.tabs
      .map((id) => {
        const tool = findTool(id);
        const name = tool?.name ?? '首页';
        return `<div class="tab ${id === this.state.active ? 'active' : ''}" role="presentation"><button role="tab" id="tab-${id}" aria-selected="${id === this.state.active}" aria-controls="panel-${id}" tabindex="${id === this.state.active ? 0 : -1}" data-tab="${id}">${tool ? toolIcon(tool, true) : '<span class="tab-home" aria-hidden="true">⌂</span>'}<span>${name}</span></button>${id !== 'home' ? `<button class="tab-close" data-close="${id}" aria-label="关闭${name}" title="关闭${name}">×</button>` : '<span class="pin" title="固定首页" aria-hidden="true">·</span>'}</div>`;
      })
      .join('');
    this.element<HTMLElement>(`#tab-${this.state.active}`).scrollIntoView({
      block: 'nearest',
      inline: 'nearest',
    });
    this.element<HTMLElement>('#footer-tool').textContent =
      findTool(this.state.active)?.name ?? '工作台首页';
    this.element<HTMLElement>('.workbench').classList.toggle(
      'nav-collapsed',
      this.state.sidebarCollapsed,
    );
    this.buttons('[data-placeholder-favorite]').forEach((button) => {
      const id = button.dataset.placeholderFavorite;
      const favorite = isToolId(id) && this.state.favorites.includes(id);
      button.textContent = favorite ? '★ 已加入常用工具' : '☆ 加入常用工具';
      button.setAttribute('aria-pressed', String(favorite));
    });
    this.element<HTMLElement>('#storage-status').textContent = this.storageAvailable
      ? '工作区自动保存到本机'
      : '当前会话模式 · 工作区无法保存';
    this.renderNavigation();
  }
  private renderNavigation(): void {
    const query = this.element<HTMLInputElement>('#tool-search').value;
    let matches = 0;
    this.buttons('.nav-item').forEach((button) => {
      const tool = findTool(button.dataset.open ?? '')!;
      button.hidden = !matchesSearch(tool, query);
      if (!button.hidden) matches++;
    });
    this.root.querySelectorAll<HTMLElement>('.nav-group').forEach((group) => {
      const category = group.dataset.category as Category;
      const items = requiredElement<HTMLElement>('.group-items', group);
      const toggle = requiredElement<HTMLButtonElement>('.group-toggle', group);
      group.hidden = ![...items.querySelectorAll<HTMLButtonElement>('.nav-item')].some(
        (button) => !button.hidden,
      );
      const expanded = query.trim() !== '' || !this.state.collapsedCategories.includes(category);
      toggle.setAttribute('aria-expanded', String(expanded));
      items.hidden = !expanded;
    });
    this.element<HTMLElement>('#no-results').hidden = matches > 0;
    this.buttons('.nav-home, .nav-item').forEach((button) => {
      const selected = button.dataset.open === this.state.active;
      button.classList.toggle('selected', selected);
      if (selected) button.setAttribute('aria-current', 'page');
      else button.removeAttribute('aria-current');
    });
  }
  private open(id: TabId): void {
    this.update(openTab(this.state, id), `#tab-${id}`);
  }
  private setTabMenu(open: boolean): void {
    this.element<HTMLElement>('#tab-dropdown').hidden = !open;
    this.element<HTMLElement>('#tab-menu').setAttribute('aria-expanded', String(open));
    if (open) this.root.querySelector<HTMLButtonElement>('#tab-dropdown button')?.focus();
  }
  private onClick = (event: MouseEvent): void => {
    if (!(event.target instanceof Element)) return;
    const button = event.target.closest<HTMLButtonElement>('button');
    if (!button) return;
    const { open, tab, close, favorite, placeholderFavorite, filter, group, action } =
      button.dataset;
    if (isTabId(open)) this.open(open);
    else if (isTabId(tab)) this.update(openTab(this.state, tab, false), `#tab-${tab}`);
    else if (isTabId(close)) {
      const nextState = closeTab(this.state, close);
      this.update(nextState, `#tab-${nextState.active}`);
    } else if (isToolId(favorite) || isToolId(placeholderFavorite)) {
      const id = (favorite ?? placeholderFavorite)!;
      if (!isToolId(id)) return;
      const adding = !this.state.favorites.includes(id);
      this.update(
        toggleFavorite(this.state, id),
        favorite ? `[data-favorite="${id}"]` : `[data-placeholder-favorite="${id}"]`,
      );
      // Removing the last visible favorite can remove its card. Keep keyboard focus in the filter.
      if (favorite && !this.root.querySelector(`[data-favorite="${id}"]`))
        this.element<HTMLElement>('[data-filter="favorites"]').focus();
      this.notify(adding ? '已加入常用工具' : '已取消收藏');
    } else if (filter === 'all' || filter === 'favorites') {
      this.update({ ...this.state, homeFilter: filter }, `[data-filter="${filter}"]`);
    } else if (group && categories.includes(group as Category)) {
      // During search all matching categories stay expanded. Clear search before toggling.
      this.element<HTMLInputElement>('#tool-search').value = '';
      const category = group as Category;
      this.update(
        {
          ...this.state,
          collapsedCategories: this.state.collapsedCategories.includes(category)
            ? this.state.collapsedCategories.filter((item) => item !== category)
            : [...this.state.collapsedCategories, category],
        },
        `[data-group="${category}"]`,
      );
    } else if (action) this.handleAction(action);
  };
  private handleAction(action: string): void {
    if (action === 'collapse-sidebar' || action === 'expand-sidebar')
      this.update(
        { ...this.state, sidebarCollapsed: action === 'collapse-sidebar' },
        `[data-action="${action === 'collapse-sidebar' ? 'expand-sidebar' : 'collapse-sidebar'}"]`,
      );
    if (action === 'clear-recent') {
      this.update({ ...this.state, recent: [] }, '#recent-title');
      this.notify('最近打开记录已清空');
    }
    if (action === 'toggle-tab-menu')
      this.setTabMenu(this.element<HTMLElement>('#tab-dropdown').hidden);
    if (action === 'close-all' || action === 'close-others') {
      this.setTabMenu(false);
      this.update(
        action === 'close-all' ? closeAllTabs(this.state) : closeOtherTabs(this.state),
        `#tab-${action === 'close-all' ? 'home' : this.state.active}`,
      );
    }
  }
  private onKeyDown = (event: KeyboardEvent): void => {
    if (!(event.target instanceof Element)) return;
    const typing = !!event.target.closest('input, textarea, select, [contenteditable="true"]');
    if (event.key === '/' && !typing && !event.metaKey && !event.ctrlKey && !event.altKey) {
      event.preventDefault();
      if (this.state.sidebarCollapsed) this.update({ ...this.state, sidebarCollapsed: false });
      this.element<HTMLInputElement>('#tool-search').focus();
    }
    if (event.altKey && event.key.toLowerCase() === 'h' && !typing) {
      event.preventDefault();
      this.open('home');
    }
    if (event.key === 'Escape') {
      if (!this.element<HTMLElement>('#tab-dropdown').hidden) {
        this.setTabMenu(false);
        this.element<HTMLElement>('#tab-menu').focus();
      }
      if (event.target.matches('#tool-search')) {
        this.element<HTMLInputElement>('#tool-search').value = '';
        this.renderNavigation();
      }
    }
    const tab = event.target.closest<HTMLButtonElement>('[role="tab"]');
    if (tab && isTabId(tab.dataset.tab)) {
      const index = this.state.tabs.indexOf(tab.dataset.tab);
      let next: TabId | undefined;
      if (event.key === 'ArrowRight') next = this.state.tabs[(index + 1) % this.state.tabs.length];
      if (event.key === 'ArrowLeft')
        next = this.state.tabs[(index - 1 + this.state.tabs.length) % this.state.tabs.length];
      if (event.key === 'Home') next = 'home';
      if (event.key === 'End') next = this.state.tabs.at(-1);
      if (next) {
        event.preventDefault();
        this.update(openTab(this.state, next, false), `#tab-${next}`);
      }
      if (event.key === 'Delete' && tab.dataset.tab !== 'home') {
        event.preventDefault();
        const nextState = closeTab(this.state, tab.dataset.tab);
        this.update(nextState, `#tab-${nextState.active}`);
      }
    }
    if (
      event.target.closest('#tab-dropdown') &&
      ['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)
    ) {
      event.preventDefault();
      const buttons = this.buttons('#tab-dropdown button');
      const index = buttons.indexOf(event.target as HTMLButtonElement);
      const next =
        event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? buttons.length - 1
            : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
      buttons[next].focus();
    }
  };
}

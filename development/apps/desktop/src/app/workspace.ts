import { categories, isTabId, isToolId, type Category, type TabId, type ToolId } from './tools.ts';

export const STORAGE_KEY = 'handykit.workspace.v2';
export const LEGACY_KEY = 'handykit.workspace.v1';
export type HomeFilter = 'all' | 'favorites';
export interface WorkspaceState {
  version: 2;
  tabs: TabId[];
  active: TabId;
  favorites: ToolId[];
  recent: ToolId[];
  collapsedCategories: Category[];
  sidebarCollapsed: boolean;
  homeFilter: HomeFilter;
}
export interface StoragePort {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}
const uniqueTools = (value: unknown): ToolId[] => Array.isArray(value) ? [...new Set(value.filter(isToolId))] : [];
export function normalizeWorkspace(value: unknown): WorkspaceState {
  const data = value && typeof value === 'object' ? value as Record<string, unknown> : {};
  const tabs: TabId[] = ['home', ...uniqueTools(data.tabs)];
  return {
    version: 2,
    tabs,
    active: isTabId(data.active) && tabs.includes(data.active) ? data.active : 'home',
    favorites: Array.isArray(data.favorites) ? uniqueTools(data.favorites) : ['json', 'timestamp', 'cron'],
    recent: uniqueTools(data.recent).slice(0, 4),
    collapsedCategories: Array.isArray(data.collapsedCategories) ? categories.filter(category => data.collapsedCategories instanceof Array && data.collapsedCategories.includes(category)) : [],
    sidebarCollapsed: data.sidebarCollapsed === true,
    homeFilter: data.homeFilter === 'favorites' ? 'favorites' : 'all',
  };
}
export function loadWorkspace(storage: StoragePort): { state: WorkspaceState; available: boolean } {
  try {
    const raw = storage.getItem(STORAGE_KEY) ?? storage.getItem(LEGACY_KEY);
    let saved: unknown;
    try { saved = raw ? JSON.parse(raw) : undefined; } catch { saved = undefined; }
    return { state: normalizeWorkspace(saved), available: true };
  } catch { return { state: normalizeWorkspace(undefined), available: false }; }
}
export function saveWorkspace(storage: StoragePort, state: WorkspaceState): boolean {
  try { storage.setItem(STORAGE_KEY, JSON.stringify(state)); return true; } catch { return false; }
}
/** These transitions never record tab focus/close as a new use of a tool. */
export function openTab(state: WorkspaceState, id: TabId, recordVisit = true): WorkspaceState {
  if (!isTabId(id)) return state;
  return {
    ...state,
    tabs: state.tabs.includes(id) ? state.tabs : [...state.tabs, id],
    active: id,
    recent: id !== 'home' && recordVisit ? [id, ...state.recent.filter(item => item !== id)].slice(0, 4) : state.recent,
  };
}
export function closeTab(state: WorkspaceState, id: TabId): WorkspaceState {
  const index = state.tabs.indexOf(id);
  if (id === 'home' || index === -1) return state;
  const tabs = state.tabs.filter(tab => tab !== id);
  return { ...state, tabs, active: state.active === id ? tabs[Math.max(0, index - 1)] : state.active };
}
export function closeOtherTabs(state: WorkspaceState): WorkspaceState {
  return { ...state, tabs: state.active === 'home' ? ['home'] : ['home', state.active] };
}
export function closeAllTabs(state: WorkspaceState): WorkspaceState {
  return { ...state, tabs: ['home'], active: 'home' };
}
export function toggleFavorite(state: WorkspaceState, id: ToolId): WorkspaceState {
  return { ...state, favorites: state.favorites.includes(id) ? state.favorites.filter(tool => tool !== id) : [...state.favorites, id] };
}

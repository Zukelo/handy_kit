import test from 'node:test';
import assert from 'node:assert/strict';
import { closeAllTabs, closeOtherTabs, closeTab, LEGACY_KEY, loadWorkspace, normalizeWorkspace, openTab, saveWorkspace, STORAGE_KEY, toggleFavorite } from './workspace.ts';
import { matchesSearch, tools } from './tools.ts';

const fresh = () => normalizeWorkspace(undefined);
test('corrupt storage and invalid tool IDs cannot break startup', () => {
  const state = normalizeWorkspace({ tabs: ['xml', 'home', 'xml', 'unknown'], active: 'unknown', favorites: ['json', 'json', 42], recent: [null, 'tasks', 'unknown'], collapsedCategories: ['unknown', '格式与校验'], sidebarCollapsed: 'true' });
  assert.deepEqual(state.tabs, ['home', 'xml']);
  assert.equal(state.active, 'home');
  assert.deepEqual(state.favorites, ['json']);
  assert.deepEqual(state.recent, ['tasks']);
  assert.deepEqual(state.collapsedCategories, ['格式与校验']);
  assert.equal(state.sidebarCollapsed, false);
  assert.equal(loadWorkspace({ getItem: () => '{invalid', setItem() {} }).state.active, 'home');
  assert.deepEqual(normalizeWorkspace(null), fresh());
});
test('legacy prototype preferences are normalized and can be saved as v2', () => {
  const records = new Map([[LEGACY_KEY, JSON.stringify({ tabs: ['xml'], active: 'xml', favorites: [] })]]);
  const storage = { getItem: (key: string) => records.get(key) ?? null, setItem: (key: string, value: string) => { records.set(key, value); } };
  const loaded = loadWorkspace(storage);
  assert.equal(loaded.state.active, 'xml');
  assert.deepEqual(loaded.state.favorites, []);
  assert.equal(saveWorkspace(storage, loaded.state), true);
  assert.equal(JSON.parse(records.get(STORAGE_KEY)!).version, 2);
});
test('denied storage falls back to a usable in-memory workspace', () => {
  const storage = { getItem(): never { throw new Error('denied'); }, setItem(): never { throw new Error('quota'); } };
  assert.equal(loadWorkspace(storage).available, false);
  assert.deepEqual(loadWorkspace(storage).state, fresh());
  assert.equal(saveWorkspace(storage, fresh()), false);
});
test('each tool has only one tab and selected tabs survive reloading', () => {
  let state = openTab(fresh(), 'json');
  state = openTab(state, 'xml');
  state = openTab(state, 'json');
  assert.deepEqual(state.tabs, ['home', 'json', 'xml']);
  assert.equal(state.active, 'json');
  assert.deepEqual(normalizeWorkspace(JSON.parse(JSON.stringify(state))), state);
});
test('closing tabs preserves home, picks a neighbor and does not change recent entries', () => {
  let state = openTab(openTab(openTab(fresh(), 'json'), 'xml'), 'cron');
  const recent = [...state.recent];
  state = closeTab(state, 'xml');
  assert.equal(state.active, 'cron');
  state = closeTab(state, 'cron');
  assert.equal(state.active, 'json');
  state = closeTab(state, 'json');
  assert.equal(state.active, 'home');
  assert.deepEqual(state.recent, recent);
  assert.equal(closeTab(state, 'home'), state);
  assert.equal(closeTab(state, 'tasks'), state);
});
test('close-other and close-all keep the pinned home', () => {
  const state = openTab(openTab(fresh(), 'json'), 'cron');
  assert.deepEqual(closeOtherTabs(state).tabs, ['home', 'cron']);
  assert.deepEqual(closeOtherTabs(openTab(state, 'home')).tabs, ['home']);
  assert.deepEqual(closeAllTabs(state).tabs, ['home']);
  assert.equal(closeAllTabs(state).active, 'home');
});
test('recent entries are capped, deduplicated, and tab focus alone is not a new visit', () => {
  let state = fresh();
  for (const tool of tools) state = openTab(state, tool.id);
  assert.equal(state.recent.length, 4);
  state = openTab(state, 'xml');
  assert.equal(state.recent[0], 'xml');
  assert.equal(new Set(state.recent).size, state.recent.length);
  assert.deepEqual(openTab(state, 'tasks', false).recent, state.recent);
});
test('favorites can all be removed and empty preferences survive normalization', () => {
  let state = fresh();
  for (const id of [...state.favorites]) state = toggleFavorite(state, id);
  assert.deepEqual(normalizeWorkspace(state).favorites, []);
  state = toggleFavorite(state, 'xml');
  assert.deepEqual(state.favorites, ['xml']);
});
test('menu search supports English, Chinese and category names', () => {
  assert.equal(matchesSearch(tools[0], 'JSON'), true);
  assert.equal(matchesSearch(tools[2], '时间戳'), true);
  assert.equal(matchesSearch(tools[2], '时间与任务'), true);
  assert.equal(matchesSearch(tools[0], 'missing'), false);
  assert.equal(matchesSearch(tools[0], '  '), true);
});

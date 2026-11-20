import test from 'node:test';
import assert from 'node:assert/strict';
import { renderHome } from './home.ts';
import { renderToolPlaceholder } from './tool-placeholder.ts';
import { normalizeWorkspace } from '../app/workspace.ts';
import { tools } from '../app/tools.ts';

test('home reflects favorites, recent tools and empty states', () => {
  const state = normalizeWorkspace({ favorites: ['xml'], recent: ['cron'], homeFilter: 'favorites' });
  const html = renderHome(state);
  assert.match(html, /data-favorite="xml"/);
  assert.doesNotMatch(html, /data-favorite="json"/);
  assert.match(html, /data-action="clear-recent"/);
  const empty = renderHome(normalizeWorkspace({ favorites: [], recent: [], homeFilter: 'favorites' }));
  assert.match(empty, /还没有常用工具/);
  assert.match(empty, /还没有收藏的工具/);
  assert.doesNotMatch(empty, /data-action="clear-recent"/);
});
test('unimplemented tools retain navigation-only placeholders', () => {
  for (const tool of tools.filter(tool => tool.id !== 'tasks')) {
    const html = renderToolPlaceholder(tool);
    assert.ok(html.includes(tool.name));
    assert.match(html, /data-open="home"/);
    assert.ok(html.includes(`data-placeholder-favorite="${tool.id}"`));
    assert.doesNotMatch(html, /<textarea|<input|<form/);
  }
});

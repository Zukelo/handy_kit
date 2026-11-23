import test from 'node:test';
import assert from 'node:assert/strict';
import { closeViews, type ClosableView } from './lifecycle.ts';

test('rejecting one tab preserves every other tab and its dirty state', async () => {
  let disposed = 0;
  const view = (allow: boolean): ClosableView => ({
    dirty: () => true,
    canClose: async () => allow,
    dispose: () => {
      disposed += 1;
    },
  });
  const first = view(true),
    second = view(false);
  assert.equal(await closeViews([first, second]), false);
  assert.equal(disposed, 0);
  assert.equal(first.dirty(), true);
  let committed = false;
  assert.equal(
    await closeViews(
      [first, view(true)],
      () => true,
      () => {
        assert.equal(disposed, 2);
        committed = true;
      },
    ),
    true,
  );
  assert.equal(committed, true);
  assert.equal(disposed, 2);
});

test('a superseded close request cannot dispose a live view', async () => {
  let current = true,
    disposed = false;
  const view: ClosableView = {
    dirty: () => true,
    canClose: async () => {
      current = false;
      return true;
    },
    dispose: () => {
      disposed = true;
    },
  };
  assert.equal(await closeViews([view], () => current), false);
  assert.equal(disposed, false);
});

import test from 'node:test';
import assert from 'node:assert/strict';
import { freshTask, generatedCron, visualFields } from './model.ts';
test('visual common periods generate unix5 without converting interval semantics', () => {
  assert.equal(generatedCron('weekly', 5, 0, '09:00', [1, 2, 3, 4, 5], '1'), '0 9 * * 1,2,3,4,5');
  assert.equal(generatedCron('minutes', 23, 0, '09:00', [], '1'), '*/23 * * * *');
  assert.equal(generatedCron('monthly', 5, 0, '18:15', [], '1,31'), '15 18 1,31 * *');
  assert.equal(generatedCron('hourly', 5, 15, '09:00', [], '1'), '15 * * * *');
});
test('field builder preserves advanced combinations instead of silently replacing them', () => {
  assert.equal(visualFields('0,15-45/5 9 * * *'), null);
  assert.deepEqual(visualFields('*/5 9-17 * * 1,3,5')?.[0], { mode: 'step', value: '5' });
});
test('new tasks are paused, empty and independent from prototype seeds', () => {
  const t = freshTask('UTC', 'macos');
  assert.equal(t.id, '');
  assert.equal(t.enabled, false);
  assert.deepEqual(t.action.args, []);
  assert.equal(t.action.target, '');
  assert.equal(freshTask('UTC', 'windows').action.interpreter, '');
});

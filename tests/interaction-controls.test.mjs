import {test} from 'node:test';
import assert from 'node:assert/strict';
import {fetchControls, interactionActive} from '../src/interaction-controls.ts';

test('holding, chasing, carrying and releasing cannot restart a toy from the panel', () => {
  for (const phase of ['held', 'chasing', 'catching', 'returning', 'teasing', 'releasing', 'rolling']) {
    for (const frisbee of [false, true]) {
      const controls = fetchControls(phase, frisbee);
      assert.equal(controls.canThrow, false);
      assert.equal(controls.canShow, false);
    }
  }
});
test('a returned toy can be thrown again without putting out a new toy', () => {
  for (const frisbee of [false, true]) {
    assert.deepEqual(fetchControls('returned', frisbee), {canShow:false, canThrow:true, throwLabel:'再扔一次'});
    assert.equal(fetchControls('ready', frisbee).canThrow, true);
    assert.equal(fetchControls('off', frisbee).canShow, true);
  }
});
test('unknown phases fail closed and feeding explains unfinished practice', () => {
  assert.equal(fetchControls('future-phase', true).canThrow, false);
  assert.equal(interactionActive('off', 'completed'), true);
  assert.equal(interactionActive('tug_ready', 'off'), true);
  assert.equal(interactionActive('off', 'blocked'), false);
});

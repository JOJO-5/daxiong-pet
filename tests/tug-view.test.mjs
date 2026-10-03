import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ropeGeometry } from '../src/tug-view.ts';

test('rope endpoints stay at their physical mouth and handle on both sides and all DPI scales', () => {
  for (const scale of [1, 1.25, 2]) for (const direction of [-1, 1]) {
    const mouth = [-1200, 450], handle = [mouth[0] + direction * 150 * scale, mouth[1] + 40 * scale];
    const rope = { mouth, handle, tension: 60, right: direction > 0 };
    // GTK can supply a larger window than requested; the centre remains the native midpoint.
    for (const [width, height] of [[194, 84], [220, 200]]) {
      const g = ropeGeometry(rope, width, height, scale);
      const origin = [(mouth[0] + handle[0]) / 2 - width * scale / 2, (mouth[1] + handle[1]) / 2 - height * scale / 2];
      assert.equal(origin[0] + g.x1 * scale, mouth[0]);
      assert.equal(origin[1] + g.y1 * scale, mouth[1]);
      assert.equal(origin[0] + g.x2 * scale, handle[0]);
      assert.equal(origin[1] + g.y2 * scale, handle[1]);
      assert.ok(Math.min(g.x1,g.x2) >= 22 && Math.max(g.x1,g.x2) <= width-22);
    }
  }
});

test('taut rope straightens while a resting rope has bounded slack', () => {
  const view = {mouth: [0,0],handle:[80,0],tension:0,right:true};
  assert.equal(ropeGeometry(view,124,44,1).sag,18);
  assert.equal(ropeGeometry({...view,tension:100},124,44,1).sag,0);
  assert.equal(ropeGeometry({...view,tension:200},124,44,1).sag,0);
});

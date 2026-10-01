import { test } from 'node:test';
import assert from 'node:assert/strict';
import { DEFAULT_SPEECH, DAXIONG_SPEECH, mergeSpeech, pickSpeech, formatSpeech } from '../src/speech.ts';
test('category history survives intervening speech and avoids recent three lines', () => {
  const history = new Map();
  const table = { click: ['a', 'b', 'c', 'd'], idle: ['idle'] };
  const spoken = [];
  for (let i = 0; i < 5; i++) {
    spoken.push(pickSpeech(table, 'click', 'idle', history, () => 0));
    pickSpeech(table, 'idle', spoken.at(-1), history, () => 0);
  }
  assert.deepEqual(spoken, ['a', 'b', 'c', 'd', 'a']);
});
test('formatted hourly messages avoid immediate repeats', () => {
  const table = { chime: ['现在 {time}', '时间 {time}'] };
  assert.equal(pickSpeech(table, 'chime', formatSpeech('现在 {time}'), new Map(), () => 0), formatSpeech('时间 {time}'));
});
test('single-line and duplicate pools stay usable with bounded history', () => {
  const history = new Map();
  for (let i = 0; i < 20; i++) assert.equal(pickSpeech({ click: ['hi', 'hi'] }, 'click', 'hi', history), 'hi');
  assert.deepEqual(history.get('click'), []);
});
test('legacy pet overrides carry over to new interaction categories', () => {
  const table = mergeSpeech({ drag: ['custom drag'], pat: ['custom pat'], throw: ['custom throw'] });
  assert.deepEqual(table.gentle_drag, ['custom drag']);
  assert.deepEqual(table.throw, ['custom throw']);
  assert.deepEqual(table.comfort, ['custom pat']);
});
test('builtin personality does not replace external pet defaults', () => {
  assert.equal(mergeSpeech(undefined), DEFAULT_SPEECH);
  assert.equal(mergeSpeech(undefined, DAXIONG_SPEECH), DAXIONG_SPEECH);
  assert.notDeepEqual(DEFAULT_SPEECH.click, DAXIONG_SPEECH.click);
  assert.equal(pickSpeech({ idle: [] }, 'unknown', null), null);
});

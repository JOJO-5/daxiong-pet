import { test } from 'node:test';
import assert from 'node:assert/strict';
import { decodePetImage, BUILTIN_SRC } from '../src/pet-image.ts';
const pet = { id: 'external', name: '外部宠物', rows: 9, data_url: 'data:image/png;base64,test', speech: { click: ['hi'] } };
const decoder = (width, height, error) => () => ({ src: '', naturalWidth: width, naturalHeight: height, decode: async () => { if (error) throw error; } });
test('missing external image uses builtin dimensions and default speech', async () => {
  const result = await decodePetImage({ ...pet, data_url: null }, decoder(1536, 3744));
  assert.deepEqual(result, { src: BUILTIN_SRC, rows: 18, speech: undefined });
});
test('nine-row pet keeps its dimensions and speech', async () => {
  const result = await decodePetImage(pet, decoder(1536, 1872));
  assert.equal(result.rows, 9);
  assert.deepEqual(result.speech, pet.speech);
});
test('corrupt image fails instead of replacing the rendered pet', async () => {
  await assert.rejects(decodePetImage(pet, decoder(0, 0, new Error('decode failed'))), /decode failed/);
});
test('changed or mismatched image dimensions are rejected', async () => {
  await assert.rejects(decodePetImage(pet, decoder(1536, 3328)), /尺寸/);
});
test('unbounded manifest rows are rejected', async () => {
  await assert.rejects(decodePetImage({ ...pet, rows: 1e9 }, decoder(1536, 1872)), /无效/);
});

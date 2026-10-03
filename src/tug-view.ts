export type TugView = { mouth: [number, number]; handle: [number, number]; tension: number; right: boolean };

/** The native window is centred between the rope ends, even on GTK with a larger minimum size. */
export function ropeGeometry(rope: TugView, width: number, height: number, scale: number) {
  const dx = (rope.handle[0] - rope.mouth[0]) / scale;
  const dy = (rope.handle[1] - rope.mouth[1]) / scale;
  return { x1: width / 2 - dx / 2, y1: height / 2 - dy / 2,
    x2: width / 2 + dx / 2, y2: height / 2 + dy / 2,
    sag: 18 * (1 - Math.max(0, Math.min(100, rope.tension)) / 100) };
}

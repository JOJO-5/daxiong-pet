import { memo, useCallback, useLayoutEffect, useRef } from "react";

const CELL_W = 192;
const CELL_H = 208;

/** Keep neighbouring atlas cells out of the compositor's visible texture. */
function PetFrame({ src, row, col }: { src: string; row: number; col: number }) {
  const source = useRef<HTMLImageElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const draw = useCallback(() => {
    const image = source.current;
    const context = canvas.current?.getContext("2d");
    if (!context) return;
    // A sparse frame must also erase the previous frame's tail and paws.
    context.clearRect(0, 0, CELL_W, CELL_H);
    if (!image?.complete || !image.naturalWidth || !image.naturalHeight) return;
    if (!Number.isInteger(row) || !Number.isInteger(col) || row < 0 || col < 0 ||
        col >= 8 || (row + 1) * CELL_H > image.naturalHeight) return;
    context.drawImage(image, col * CELL_W, row * CELL_H, CELL_W, CELL_H,
      0, 0, CELL_W, CELL_H);
  }, [src, row, col]);

  useLayoutEffect(draw, [draw]);

  return <>
    {/* Retain the decoded source for loading/legacy-pack diagnostics only. */}
    <img ref={source} className="pet-sheet" src={src} alt="" hidden
      draggable={false} onLoad={draw} onError={draw} />
    <canvas ref={canvas} className="pet-frame" width={CELL_W} height={CELL_H}
      aria-hidden="true" />
  </>;
}

export default memo(PetFrame);

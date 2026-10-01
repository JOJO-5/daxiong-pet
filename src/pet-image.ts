export const BUILTIN_SRC = "/spritesheet-extended.webp";
export const DEFAULT_ROWS = 16;
export type PetSwitch = {
  id: string;
  name: string;
  rows: number;
  data_url: string | null;
  speech?: unknown;
};
type ImageDecoder = Pick<HTMLImageElement, "src" | "decode" | "naturalWidth" | "naturalHeight">;

/** Decode and validate before replacing the currently displayed pet. */
export async function decodePetImage(
  pet: PetSwitch,
  createImage: () => ImageDecoder = () => new Image(),
): Promise<{ src: string; rows: number; speech?: unknown }> {
  const builtin = pet.data_url === null;
  const src = builtin ? BUILTIN_SRC : pet.data_url;
  const rows = builtin ? DEFAULT_ROWS : pet.rows;
  if (!Number.isInteger(rows) || rows < 9 || rows > 256 || !src) {
    throw new Error("宠物图集信息无效");
  }
  const image = createImage();
  image.src = src;
  await image.decode();
  if (image.naturalWidth !== 1536 || image.naturalHeight !== rows * 208) {
    throw new Error("宠物图集尺寸与配置不符");
  }
  return { src, rows, speech: builtin ? undefined : pet.speech };
}

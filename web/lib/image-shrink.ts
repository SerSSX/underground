import { shrinkPlan } from "./images.mjs";

function toBlob(canvas: HTMLCanvasElement, type: string, quality: number): Promise<Blob | null> {
  return new Promise((resolve) => canvas.toBlob(resolve, type, quality));
}

/** Shrink a big photo in the browser before upload (TASK-304): ≤ 1600 px, WebP where the browser can write it
 *  (Safari cannot: JPEG then; a PNG stays as it is so transparency survives). Upright by its EXIF orientation.
 *  Any failure, or a result no smaller than the original, sends the original — the API re-encodes it anyway. */
export async function shrinkForUpload(file: File): Promise<File> {
  if (typeof window === "undefined" || typeof createImageBitmap !== "function") return file;
  let bitmap: ImageBitmap | null = null;
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
    const plan = shrinkPlan({ size: file.size, type: file.type, width: bitmap.width, height: bitmap.height });
    if (!plan) return file;
    const canvas = document.createElement("canvas");
    canvas.width = plan.width;
    canvas.height = plan.height;
    const ctx = canvas.getContext("2d");
    if (!ctx) return file;
    ctx.imageSmoothingQuality = "high";
    ctx.drawImage(bitmap, 0, 0, plan.width, plan.height);
    let blob = await toBlob(canvas, "image/webp", 0.85);
    if (!blob || blob.type !== "image/webp") {
      if (file.type === "image/png") return file;
      blob = await toBlob(canvas, "image/jpeg", 0.88);
    }
    if (!blob || blob.size >= file.size) return file;
    const ext = blob.type === "image/webp" ? "webp" : "jpg";
    return new File([blob], `${file.name.replace(/\.[^.]*$/, "") || "photo"}.${ext}`, { type: blob.type });
  } catch {
    return file;
  } finally {
    bitmap?.close();
  }
}

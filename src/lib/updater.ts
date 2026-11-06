import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

/** Returns the pending update, or null when already up to date. */
export function findUpdate(): Promise<Update | null> {
  return check();
}

export async function installUpdate(
  update: Update,
  onProgress?: (downloaded: number, total?: number) => void,
): Promise<void> {
  let downloaded = 0;
  let total: number | undefined;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") total = event.data.contentLength;
    if (event.event === "Progress") {
      downloaded += event.data.chunkLength;
      onProgress?.(downloaded, total);
    }
  });
  await relaunch();
}

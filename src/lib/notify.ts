import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from "@tauri-apps/plugin-notification";
import type { Alert } from "#lib/api.ts";

// asked once per run, the os remembers the answer
let allowed: Promise<boolean> | null = null;

async function askPermission() {
  if (await isPermissionGranted()) return true;
  return (await requestPermission()) === "granted";
}

export async function notifyAlert(alert: Alert) {
  // the list on screen is enough while the user is looking at it
  if (await getCurrentWindow().isFocused()) return;
  allowed ??= askPermission();
  if (!(await allowed)) return;
  sendNotification({
    title: `${alert.itemName}, ${alert.platinum}p`,
    body: `${alert.trader} - open NasuaFrame to copy the whisper`,
  });
}

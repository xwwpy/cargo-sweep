import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { errorMessage, isDemoMode, isTauriRuntime } from "../api";
import { APP_VERSION } from "../lib/version";

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  date?: string;
  body?: string;
}

const availableUpdate = ref<UpdateInfo | null>(null);
const dismissed = ref(false);
const checking = ref(false);
const installing = ref(false);
const downloaded = ref(0);
const contentLength = ref(0);
const preparingRestart = ref(false);
let pendingUpdate: Update | null = null;

const hasUpdate = computed(() => availableUpdate.value !== null);
const progressPercent = computed(() => {
  if (contentLength.value <= 0) {
    return 0;
  }
  return Math.min(100, Math.round((downloaded.value / contentLength.value) * 100));
});

async function checkWithRetry(): Promise<Update | null> {
  let lastError: unknown;
  for (let attempt = 1; attempt <= 3; attempt += 1) {
    try {
      return await check({
        timeout: 20_000,
        headers: {
          "Cache-Control": "no-cache",
          Pragma: "no-cache",
        },
      });
    } catch (error) {
      lastError = error;
      if (attempt < 3) {
        await new Promise((resolve) => window.setTimeout(resolve, attempt * 800));
      }
    }
  }
  throw lastError;
}

async function checkForUpdate(options: { silent?: boolean } = {}): Promise<UpdateInfo | null> {
  if (!isTauriRuntime()) {
    if (isDemoMode() && new URLSearchParams(window.location.search).has("update")) {
      availableUpdate.value = {
        version: "0.5.7",
        currentVersion: APP_VERSION,
        date: new Date().toISOString(),
        body: "新增：GitHub Releases 自动更新\n优化：全新 Ink & Mint 界面配色\n修复：部分 Windows 临时目录导致安装失败的问题\n\n更新完成后应用会自动重启。",
      };
      dismissed.value = false;
      return availableUpdate.value;
    }
    if (!options.silent) {
      ElMessage.info("自动更新仅在桌面版中可用");
    }
    return null;
  }
  if (checking.value || installing.value) {
    return availableUpdate.value;
  }

  checking.value = true;
  try {
    const update = await checkWithRetry();
    if (!update) {
      availableUpdate.value = null;
      pendingUpdate = null;
      dismissed.value = false;
      if (!options.silent) {
        ElMessage.success(`当前已是最新版本 v${APP_VERSION}`);
      }
      return null;
    }

    pendingUpdate = update;
    availableUpdate.value = {
      version: update.version,
      currentVersion: update.currentVersion,
      date: update.date,
      body: update.body,
    };
    dismissed.value = false;
    return availableUpdate.value;
  } catch (error) {
    if (!options.silent) {
      ElMessage.error(`检查更新失败：${errorMessage(error)}`);
    }
    return null;
  } finally {
    checking.value = false;
  }
}

async function installUpdate() {
  if (!pendingUpdate || installing.value) {
    return;
  }

  installing.value = true;
  downloaded.value = 0;
  contentLength.value = 0;
  preparingRestart.value = false;

  try {
    await pendingUpdate.downloadAndInstall((event) => {
      if (event.event === "Started") {
        contentLength.value = event.data.contentLength ?? 0;
      } else if (event.event === "Progress") {
        downloaded.value += event.data.chunkLength;
      } else if (event.event === "Finished") {
        downloaded.value = contentLength.value || downloaded.value;
        preparingRestart.value = true;
      }
    });

    preparingRestart.value = true;
    await relaunch();
  } catch (error) {
    installing.value = false;
    ElMessage.error(`更新安装失败：${errorMessage(error)}`);
  }
}

function dismissUpdate() {
  dismissed.value = true;
}

function clearUpdate() {
  availableUpdate.value = null;
  dismissed.value = false;
  pendingUpdate = null;
}

export function useUpdater() {
  return {
    availableUpdate,
    dismissed,
    hasUpdate,
    checking,
    installing,
    downloaded,
    contentLength,
    progressPercent,
    preparingRestart,
    checkForUpdate,
    installUpdate,
    dismissUpdate,
    clearUpdate,
  };
}

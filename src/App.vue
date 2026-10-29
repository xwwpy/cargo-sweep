<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { listen } from "@tauri-apps/api/event";
import {
  Check,
  Close,
  Download,
  Loading,
  Monitor,
  Moon,
  Plus,
  Refresh,
  Sunny,
  Ticket,
} from "@element-plus/icons-vue";
import SidebarPanel from "./components/SidebarPanel.vue";
import SummaryCards from "./components/SummaryCards.vue";
import TargetTable from "./components/TargetTable.vue";
import DeleteDialog from "./components/DeleteDialog.vue";
import RustEnvironmentPage from "./components/RustEnvironmentPage.vue";
import TasksPage from "./components/TasksPage.vue";
import UpdateBanner from "./components/UpdateBanner.vue";
import UpdateDialog from "./components/UpdateDialog.vue";
import {
  colorPalettes,
  useTheme,
  type ColorPalette,
  type LayoutStyle,
  type ThemeMode,
} from "./composables/useTheme";
import { useUpdater } from "./composables/useUpdater";
import { operationBusy } from "./composables/useOperationBusy";
import {
  activeTaskCount,
  activeTasks,
  hasRunningDeleteTask,
  startTask,
} from "./composables/useTasks";
import {
  defaultScanRoots,
  deleteTargets,
  errorMessage,
  isDemoMode,
  isTauriRuntime,
  listenToScanProgress,
  openFolder,
  openPrivacySettings,
  pickFolders,
  probeRustInstallation,
  scanTargets,
  setOperationActive,
} from "./api";
import { basename, cleanPath, formatBytes, pathKey } from "./lib/format";
import { APP_VERSION } from "./lib/version";
import type { ScanProgress, TargetInfo } from "./types";

const STORAGE_KEY = "cargo-sweep.scan-roots.v1";

const {
  mode: themeMode,
  palette: themePalette,
  layout: layoutStyle,
  isDark,
  setMode: setThemeMode,
  setPalette: setThemePalette,
  setLayout: setThemeLayout,
} = useTheme();
const paletteLabel = computed(
  () =>
    colorPalettes.find((palette) => palette.id === themePalette.value)?.name ??
    "玫瑰星云",
);
const layoutLabel = computed(() =>
  layoutStyle.value === "minimal" ? "简约布局" : "经典布局",
);
const themeLabel = computed(() => {
  const modeLabel =
    themeMode.value === "system"
      ? "跟随系统"
      : themeMode.value === "dark"
        ? "深色"
        : "浅色";
  return `${modeLabel} · ${paletteLabel.value}`;
});

const activePage = ref<"targets" | "rust" | "tasks">("targets");
const rustInstalled = ref(false);
const isMac = /Mac/i.test(navigator.userAgent);
const pageTitle = computed(
  () =>
    ({
      targets: "空间总览",
      rust: "Rust 环境",
      tasks: "任务中心",
    })[activePage.value],
);
const mainRef = ref<HTMLElement | null>(null);
const roots = ref<string[]>([]);
const targets = ref<TargetInfo[]>([]);
const selectedRows = ref<TargetInfo[]>([]);
const search = ref("");

const loading = ref(true);
const scanning = ref(false);
const deleting = hasRunningDeleteTask;
const hasScanned = ref(false);

const lastScanAt = ref<number | null>(null);
const lastScanDuration = ref(0);
const scannedDirs = ref(0);
const scannedRoots = ref(0);
const progress = ref<ScanProgress | null>(null);

const deleteDialogVisible = ref(false);
const updateDialogVisible = ref(false);
const pendingDeletion = ref<TargetInfo[]>([]);
const tableRef = ref<{ clearSelection: () => void } | null>(null);
const {
  availableUpdate,
  dismissed: updateDismissed,
  hasUpdate,
  checking: checkingUpdates,
  installing: updating,
  downloaded,
  contentLength,
  progressPercent,
  preparingRestart,
  checkForUpdate,
  installUpdate,
  dismissUpdate,
} = useUpdater();

let unlistenProgress: (() => void) | undefined;
let unlistenCloseBlocked: (() => void) | undefined;
let updateTimer: number | undefined;

watch(
  [operationBusy, activeTaskCount],
  ([foreground, background]) => {
    void setOperationActive(Boolean(foreground) || background > 0).catch(() => {});
  },
  { flush: "sync" },
);

const demoMode = isDemoMode();

const totalBytes = computed(() =>
  targets.value.reduce((total, target) => total + target.sizeBytes, 0),
);
const totalFiles = computed(() =>
  targets.value.reduce((total, target) => total + target.fileCount, 0),
);
const selectedBytes = computed(() =>
  selectedRows.value.reduce((total, target) => total + target.sizeBytes, 0),
);
const selectedCount = computed(() => selectedRows.value.length);

const filteredTargets = computed(() => {
  const keyword = search.value.trim().toLowerCase();
  if (!keyword) {
    return targets.value;
  }
  return targets.value.filter(
    (target) =>
      target.projectName.toLowerCase().includes(keyword) ||
      target.path.toLowerCase().includes(keyword) ||
      target.projectPath.toLowerCase().includes(keyword),
  );
});

const syncLabel = computed(() => {
  if (scanning.value) {
    return "扫描中";
  }
  if (lastScanAt.value) {
    return "已同步";
  }
  return "待扫描";
});

function updateRustInstallationState(installed: boolean) {
  rustInstalled.value = installed;
}

onMounted(async () => {
  if (isTauriRuntime()) {
    try {
      unlistenCloseBlocked = await listen("operation-close-blocked", () => {
        ElMessage.warning(
          "后台任务进行中，请等待完成或在任务中心取消后再关闭应用",
        );
      });
    } catch {
      // The backend still prevents the close; only the warning is optional.
    }
  }

  void probeRustInstallation()
    .then((probe) => updateRustInstallationState(probe.installed))
    .catch(() => updateRustInstallationState(false));

  try {
    unlistenProgress = await listenToScanProgress((payload) => {
      progress.value = payload;
    });
  } catch {
    // Progress events are a nicety; scanning still works without them.
  }

  await initialize();

  updateTimer = window.setTimeout(() => {
    void checkForUpdate({ silent: true });
  }, 3200);
});

onUnmounted(() => {
  unlistenProgress?.();
  unlistenCloseBlocked?.();
  void setOperationActive(false).catch(() => {});
  if (updateTimer !== undefined) {
    window.clearTimeout(updateTimer);
  }
});

async function initialize() {
  await restoreRoots();

  loading.value = false;
  if (roots.value.length > 0) {
    await runScan();
  }
}

async function restoreRoots() {
  const saved = localStorage.getItem(STORAGE_KEY);
  if (saved) {
    try {
      const parsed = JSON.parse(saved) as unknown;
      if (Array.isArray(parsed)) {
        roots.value = uniqueRoots(parsed.filter((item): item is string => typeof item === "string"));
      }
    } catch {
      localStorage.removeItem(STORAGE_KEY);
    }
  }

  if (roots.value.length > 0) {
    return;
  }

  try {
    const defaults = await defaultScanRoots();
    if (defaults.length > 0) {
      roots.value = uniqueRoots(defaults);
      persistRoots();
    }
  } catch (error) {
    ElMessage.error(`读取默认扫描目录失败：${errorMessage(error)}`);
  }
}

async function addRoots() {
  if (scanning.value) {
    return;
  }
  if (demoMode) {
    ElMessage.info("浏览器演示模式使用内置数据；桌面版可直接选择文件夹。");
    return;
  }

  try {
    const picked = await pickFolders();
    if (picked.length === 0) {
      return;
    }

    const next = uniqueRoots([...roots.value, ...picked]);
    const added = next.length - roots.value.length;
    if (added === 0) {
      ElMessage.info("这些目录已经在扫描范围中");
      return;
    }

    roots.value = next;
    persistRoots();
    ElMessage.success(`已添加 ${added} 个扫描目录`);
    await runScan();
  } catch (error) {
    ElMessage.error(`添加目录失败：${errorMessage(error)}`);
  }
}

async function removeRoot(root: string) {
  if (scanning.value) {
    return;
  }

  roots.value = roots.value.filter((item) => item !== root);
  persistRoots();
  selectedRows.value = [];
  tableRef.value?.clearSelection();

  if (roots.value.length === 0) {
    targets.value = [];
    lastScanAt.value = null;
    hasScanned.value = false;
    ElMessage.info("扫描范围已清空");
    return;
  }

  ElMessage.info(`已移除 ${basename(root)}，正在重新扫描`);
  await runScan();
}

async function runScan() {
  if (scanning.value) {
    return;
  }
  if (roots.value.length === 0) {
    ElMessage.warning("请先添加至少一个扫描目录");
    return;
  }

  const isRescan = hasScanned.value;
  scanning.value = true;
  progress.value = null;
  selectedRows.value = [];
  tableRef.value?.clearSelection();

  try {
    const result = await scanTargets(roots.value);

    targets.value = result.targets;
    lastScanAt.value = Date.now();
    lastScanDuration.value = result.elapsedMs;
    scannedDirs.value = result.scannedDirs;
    scannedRoots.value = result.scannedRoots;
    hasScanned.value = true;

    if (result.skippedRoots.length > 0) {
      ElMessage.warning(
        `${result.skippedRoots.length} 个根目录无法访问，已跳过：${basename(result.skippedRoots[0])}`,
      );
    } else if (isRescan) {
      ElMessage.success(
        result.targets.length > 0
          ? `扫描完成，共发现 ${result.targets.length} 个 target`
          : "扫描完成，当前没有可清理的 target 目录",
      );
    }
  } catch (error) {
    ElMessage.error(`扫描失败：${errorMessage(error)}`);
  } finally {
    scanning.value = false;
    progress.value = null;
  }
}

function requestDelete(rows: TargetInfo[]) {
  if (rows.length === 0 || scanning.value || deleting.value) {
    return;
  }

  const unique = [...new Map(rows.map((row) => [pathKey(row.path), row])).values()];
  pendingDeletion.value = unique;
  deleteDialogVisible.value = true;
}

function requestDeleteSelected() {
  requestDelete(selectedRows.value);
}

function requestDeleteAll() {
  requestDelete(targets.value);
}

function confirmDelete() {
  if (deleting.value || pendingDeletion.value.length === 0) {
    return;
  }

  const pending = [...pendingDeletion.value];
  const paths = pending.map((target) => target.path);
  deleteDialogVisible.value = false;
  pendingDeletion.value = [];

  startTask({
    kind: "delete",
    title: `清理 ${pending.length} 个 target 目录`,
    detail: pending
      .slice(0, 3)
      .map((target) => target.projectName)
      .join("、"),
    page: "targets",
    run: (taskId) => deleteTargets(paths, taskId),
    onFinished: (result, error) => {
      if (error) {
        ElMessage.error(`清理失败：${errorMessage(error)}`);
        return;
      }
      if (!result) {
        return;
      }
      const deletedKeys = new Set(result.deleted.map(pathKey));
      if (result.deleted.length > 0) {
        targets.value = targets.value.filter(
          (target) => !deletedKeys.has(pathKey(target.path)),
        );
        selectedRows.value = selectedRows.value.filter(
          (target) => !deletedKeys.has(pathKey(target.path)),
        );
        tableRef.value?.clearSelection();
      }
      if (result.cancelled) {
        ElMessage.info(
          `清理已取消，已释放 ${formatBytes(result.freedBytes)}，清理了 ${result.deleted.length} 个目录`,
        );
      } else if (result.deleted.length > 0) {
        ElMessage.success(
          `已释放 ${formatBytes(result.freedBytes)}，清理了 ${result.deleted.length} 个 target 目录`,
        );
      }
      if (result.failures.length > 0) {
        ElMessage.warning(
          `${result.failures.length} 个目录未能删除：${result.failures[0].message}`,
        );
      }
      if (
        !result.cancelled &&
        result.deleted.length === 0 &&
        result.failures.length === 0
      ) {
        ElMessage.info("没有目录需要删除");
      }
    },
  });
}

function changeTheme(command: string) {
  setThemeMode(command as ThemeMode);
}

function choosePalette(nextPalette: ColorPalette) {
  setThemePalette(nextPalette);
}

function chooseLayout(nextLayout: LayoutStyle) {
  setThemeLayout(nextLayout);
}

async function allowReadAccess() {
  try {
    await openPrivacySettings();
    ElMessage.info("已打开系统设置，请将 Cargo Sweep 加入「完全磁盘访问权限」并重启应用");
  } catch (reason) {
    ElMessage.error(errorMessage(reason));
  }
}

function navigateTo(page: "targets" | "rust" | "tasks") {
  if (operationBusy.value && page !== activePage.value) {
    ElMessage.warning("Rust 操作进行中，请等待完成后再切换页面");
    return;
  }
  if (page !== "targets") {
    selectedRows.value = [];
    tableRef.value?.clearSelection();
  }
  activePage.value = page;
  requestAnimationFrame(() => mainRef.value?.scrollTo({ top: 0, behavior: "smooth" }));
}

function handleSelection(rows: TargetInfo[]) {
  selectedRows.value = rows;
}

async function openTargetLocation(target: TargetInfo) {
  try {
    await openFolder(cleanPath(target.path));
  } catch (error) {
    ElMessage.error(`打开目录失败：${errorMessage(error)}`);
  }
}

async function manualCheckUpdate() {
  if (hasUpdate.value) {
    updateDialogVisible.value = true;
    return;
  }
  const info = await checkForUpdate();
  if (info) {
    updateDialogVisible.value = true;
  }
}

function openUpdateDialog() {
  if (hasUpdate.value) {
    updateDialogVisible.value = true;
  }
}

function installAvailableUpdate() {
  if (!hasUpdate.value) {
    return;
  }
  if (operationBusy.value) {
    ElMessage.warning("工具链操作进行中，请等待完成后再更新应用");
    return;
  }
  updateDialogVisible.value = true;
  void installUpdate();
}

function updateSearch(value: string) {
  search.value = value;
}

function persistRoots() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(roots.value));
  } catch {
    // The app still works when storage is unavailable.
  }
}

function uniqueRoots(list: string[]): string[] {
  const seen = new Set<string>();
  const result: string[] = [];
  for (const item of list) {
    const trimmed = item.replace(/[\\/]+$/, "");
    const key = pathKey(trimmed);
    if (!key || seen.has(key)) {
      continue;
    }
    seen.add(key);
    result.push(trimmed);
  }
  return result;
}
</script>

<template>
  <div class="app-shell" :class="{ minimal: layoutStyle === 'minimal' }">
    <header v-if="layoutStyle === 'minimal'" class="minimal-header">
      <button class="minimal-brand" type="button" @click="navigateTo('targets')">
        <img src="/logo.svg" alt="" />
        <span>
          <strong>Cargo Sweep</strong>
          <small>MINIMAL</small>
        </span>
      </button>
      <nav class="minimal-nav" aria-label="主导航">
        <button
          type="button"
          :class="{ active: activePage === 'targets' }"
          @click="navigateTo('targets')"
        >
          空间清理
        </button>
        <button
          type="button"
          :class="{ active: activePage === 'rust' }"
          @click="navigateTo('rust')"
        >
          Rust 环境
        </button>
        <button
          type="button"
          :class="{ active: activePage === 'tasks' }"
          @click="navigateTo('tasks')"
        >
          任务中心
          <span v-if="activeTaskCount > 0" class="minimal-task-count">
            {{ activeTaskCount }}
          </span>
        </button>
      </nav>
      <div class="minimal-meta">
        <span>{{ pageTitle }}</span>
        <strong>v{{ APP_VERSION }}</strong>
      </div>
    </header>

    <SidebarPanel
      v-if="layoutStyle === 'classic'"
      :roots="roots"
      :scanning="scanning"
      :target-count="targets.length"
      :total-bytes="totalBytes"
      :version="APP_VERSION"
      :update-available="hasUpdate"
      :checking-updates="checkingUpdates"
      :active-page="activePage"
      :rust-installed="rustInstalled"
      :task-count="activeTaskCount"
      :is-mac="isMac"
      @add="addRoots"
      @remove="removeRoot"
      @check-update="manualCheckUpdate"
      @navigate="navigateTo"
      @allow-read="allowReadAccess"
    />

    <main ref="mainRef" class="main-content">
      <div class="topbar">
        <div class="breadcrumb">
          <span>工作台</span>
          <i>/</i>
          <strong>{{ pageTitle }}</strong>
        </div>

        <div class="topbar-actions">
          <span v-if="activePage === 'targets'" class="sync-state" :class="{ busy: scanning }">
            <i class="sync-dot" />
            {{ syncLabel }}
          </span>

          <el-popover
            class="theme-dropdown"
            trigger="click"
            placement="bottom-end"
            :width="318"
            popper-class="theme-popper"
          >
            <template #reference>
              <button
                class="theme-toggle"
                type="button"
                :title="`主题与配色：${themeLabel}`"
                :aria-label="`切换主题与配色，当前${themeLabel}`"
              >
                <el-icon>
                  <Moon v-if="isDark" />
                  <Sunny v-else />
                </el-icon>
              </button>
            </template>

            <div class="theme-studio">
              <div class="theme-studio-head">
                <div>
                  <span class="theme-studio-kicker">APPEARANCE</span>
                  <strong>界面风格</strong>
                </div>
                <span class="theme-current">{{ themeLabel }}</span>
              </div>

              <div class="appearance-modes">
                <button
                  type="button"
                  :class="{ active: themeMode === 'light' }"
                  @click="changeTheme('light')"
                >
                  <el-icon><Sunny /></el-icon>
                  浅色
                </button>
                <button
                  type="button"
                  :class="{ active: themeMode === 'dark' }"
                  @click="changeTheme('dark')"
                >
                  <el-icon><Moon /></el-icon>
                  深色
                </button>
                <button
                  type="button"
                  :class="{ active: themeMode === 'system' }"
                  @click="changeTheme('system')"
                >
                  <el-icon><Monitor /></el-icon>
                  系统
                </button>
              </div>

              <div class="palette-heading layout-heading">
                <span>布局风格</span>
                <small>{{ layoutLabel }}</small>
              </div>
              <div class="layout-modes">
                <button
                  type="button"
                  :class="{ active: layoutStyle === 'classic' }"
                  @click="chooseLayout('classic')"
                >
                  <span class="layout-preview classic-preview" />
                  <span>
                    <strong>经典</strong>
                    <small>侧栏 · 玻璃质感</small>
                  </span>
                </button>
                <button
                  type="button"
                  :class="{ active: layoutStyle === 'minimal' }"
                  @click="chooseLayout('minimal')"
                >
                  <span class="layout-preview minimal-preview" />
                  <span>
                    <strong>简约</strong>
                    <small>顶部导航 · 平面</small>
                  </span>
                </button>
              </div>

              <div class="palette-heading">
                <span>配色方案</span>
                <small>{{ colorPalettes.length }} 种</small>
              </div>
              <div class="palette-grid">
                <button
                  v-for="palette in colorPalettes"
                  :key="palette.id"
                  type="button"
                  class="palette-card"
                  :class="{ active: themePalette === palette.id }"
                  @click="choosePalette(palette.id)"
                >
                  <span
                    class="palette-swatch"
                    :style="{
                      background: `linear-gradient(135deg, ${palette.colors[0]}, ${palette.colors[1]} 58%, ${palette.colors[2]})`,
                    }"
                  />
                  <span class="palette-copy">
                    <strong>{{ palette.name }}</strong>
                    <small>{{ palette.description }}</small>
                  </span>
                  <el-icon v-if="themePalette === palette.id" class="palette-check">
                    <Check />
                  </el-icon>
                </button>
              </div>
            </div>
          </el-popover>

          <el-tooltip
            :content="hasUpdate ? `发现新版本 v${availableUpdate?.version}` : '检查更新'"
            placement="bottom"
            :show-after="450"
          >
            <button
              class="version-chip"
              :class="{ available: hasUpdate, checking: checkingUpdates }"
              type="button"
              :disabled="checkingUpdates"
              @click="manualCheckUpdate"
            >
              <el-icon :class="{ spinning: checkingUpdates }">
                <Loading v-if="checkingUpdates" />
                <Download v-else />
              </el-icon>
              <span>v{{ APP_VERSION }}</span>
              <i v-if="hasUpdate" class="version-dot" />
            </button>
          </el-tooltip>

          <template v-if="activePage === 'targets'">
            <el-button
              class="header-button add-folder-button"
              :disabled="scanning"
              @click="addRoots"
            >
              <el-icon><Plus /></el-icon>
              添加目录
            </el-button>

            <el-button
              type="primary"
              class="header-button scan-button"
              :loading="scanning"
              :disabled="roots.length === 0"
              @click="runScan"
            >
              <el-icon v-if="!scanning"><Refresh /></el-icon>
              {{ scanning ? "扫描中" : "重新扫描" }}
            </el-button>
          </template>
        </div>
      </div>

      <div
        v-if="layoutStyle === 'minimal' && activePage === 'targets'"
        class="minimal-scope"
      >
        <span class="minimal-scope-label">扫描范围</span>
        <div class="minimal-root-chips">
          <span v-for="root in roots" :key="root" class="minimal-root">
            <span>{{ basename(root) }}</span>
            <button
              type="button"
              :disabled="scanning"
              aria-label="移除扫描目录"
              @click="removeRoot(root)"
            >
              <el-icon><Close /></el-icon>
            </button>
          </span>
          <button
            type="button"
            class="minimal-add-root"
            :disabled="scanning"
            @click="addRoots"
          >
            <el-icon><Plus /></el-icon>
            添加目录
          </button>
          <button
            v-if="isMac"
            type="button"
            class="minimal-add-root"
            @click="allowReadAccess"
          >
            允许读取
          </button>
        </div>
      </div>

      <Transition name="page" mode="out-in">
        <div :key="activePage" class="page-stage">
          <template v-if="activePage === 'targets'">
        <header class="hero">
        <div class="hero-copy">
          <span class="eyebrow"><i /> CARGO TARGET CLEANER</span>
          <h1>清理构建缓存，<span>让磁盘轻一点。</span></h1>
          <p>扫描本地 Rust 项目的 target 目录，安全回收构建缓存。</p>
        </div>
        <div class="hero-safety">
          <span><i /> 只读扫描</span>
          <span><i /> 安全校验</span>
          <span><i /> 递归 10 层</span>
        </div>
      </header>

      <div v-if="hasUpdate && !updateDismissed" class="update-slot">
        <UpdateBanner
          :version="availableUpdate?.version ?? ''"
          :body="availableUpdate?.body"
          @open="openUpdateDialog"
          @dismiss="dismissUpdate"
        />
      </div>

      <SummaryCards
        :total-bytes="totalBytes"
        :target-count="targets.length"
        :file-count="totalFiles"
        :last-scan-at="lastScanAt"
        :last-scan-duration="lastScanDuration"
        :scanned-dirs="scannedDirs"
        :scanned-roots="scannedRoots"
        :scanning="scanning"
      />

      <section class="targets-section">
        <div class="section-heading">
          <div class="section-title">
            <span class="section-kicker">PROJECTS</span>
            <h2>
              Target 目录
              <span class="title-count">{{ targets.length }}</span>
            </h2>
          </div>
        </div>

        <TargetTable
          ref="tableRef"
          :targets="filteredTargets"
          :total-targets="targets.length"
          :scanning="scanning"
          :loading="loading"
          :deleting="deleting"
          :search="search"
          :selected-count="selectedCount"
          :selected-bytes="selectedBytes"
          :progress="progress"
          @update:search="updateSearch"
          @selection-change="handleSelection"
          @request-delete="requestDelete"
          @request-delete-selected="requestDeleteSelected"
          @request-delete-all="requestDeleteAll"
          @open-location="openTargetLocation"
          @add="addRoots"
        />
        </section>
          </template>

          <RustEnvironmentPage
            v-else-if="activePage === 'rust'"
            :rust-installed="rustInstalled"
            @rust-state="updateRustInstallationState"
          />
          <TasksPage v-else />
        </div>
      </Transition>

      <footer class="app-footer">
        <span class="footer-brand">Cargo Sweep · github.com/xwwpy/cargo-sweep</span>
        <span v-if="activePage === 'targets'" class="footer-note">
          <i />
          校验 Cargo.toml · 跳过符号链接
        </span>
        <span v-else class="footer-note">
          <i />
          Rust 工具链、下载源与存储集中管理
        </span>
      </footer>
    </main>

    <Transition name="task-dock">
      <aside
        v-if="activeTasks.length > 0 && activePage !== 'tasks'"
        class="task-dock"
      >
        <button
          type="button"
          class="task-dock-main"
          @click="navigateTo('tasks')"
        >
          <span class="task-dock-head">
            <i class="task-dock-dot" />
            {{ activeTasks.length }} 个后台任务
            <strong>{{ activeTasks[0].percent }}%</strong>
          </span>
          <span class="task-dock-title">{{ activeTasks[0].title }}</span>
          <span class="task-dock-message">
            {{ activeTasks[0].message }}
          </span>
          <el-progress
            :percentage="activeTasks[0].percent"
            :show-text="false"
            :stroke-width="4"
            :indeterminate="
              activeTasks[0].percent === 0 &&
              activeTasks[0].status !== 'cancelling'
            "
            :duration="1.8"
          />
        </button>
        <button
          type="button"
          class="task-dock-open"
          @click="navigateTo('tasks')"
        >
          任务中心
          <el-icon><Ticket /></el-icon>
        </button>
      </aside>
    </Transition>

    <DeleteDialog
      v-model="deleteDialogVisible"
      :targets="pendingDeletion"
      :loading="deleting"
      @confirm="confirmDelete"
    />

    <UpdateDialog
      v-model="updateDialogVisible"
      :update="availableUpdate"
      :installing="updating"
      :preparing-restart="preparingRestart"
      :progress-percent="progressPercent"
      :downloaded="downloaded"
      :content-length="contentLength"
      @install="installAvailableUpdate"
    />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  width: 100%;
  min-height: 100vh;
  overflow: hidden;
  background:
    radial-gradient(circle at 88% -10%, rgba(45, 212, 191, 0.13), transparent 34%),
    radial-gradient(circle at 28% 112%, rgba(15, 118, 110, 0.06), transparent 36%),
    #f3f5f7;
}

.main-content {
  flex: 1;
  min-width: 0;
  height: 100vh;
  padding: 22px 30px 16px;
  overflow-x: hidden;
  overflow-y: auto;
}

.topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 32px;
}

.breadcrumb {
  display: flex;
  gap: 9px;
  align-items: center;
  font-size: 10.5px;
  color: #98a2b3;
}

.breadcrumb i {
  font-style: normal;
  color: #d0d5dd;
}

.breadcrumb strong {
  font-weight: 600;
  color: #475467;
}

.topbar-actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.theme-dropdown {
  line-height: 0;
}

.theme-toggle {
  display: grid;
  width: 27px;
  height: 27px;
  padding: 0;
  font-size: 12px;
  color: #667085;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.76);
  border: 1px solid #e4e7ec;
  border-radius: 8px;
  transition:
    color 160ms ease,
    background 160ms ease,
    border-color 160ms ease;
  place-items: center;
}

.theme-toggle:hover {
  color: #0f766e;
  background: #fff;
  border-color: #a7e5da;
}

.sync-state {
  display: flex;
  gap: 7px;
  align-items: center;
  padding: 5px 9px;
  font-size: 9.5px;
  font-weight: 500;
  color: #667085;
  background: rgba(255, 255, 255, 0.76);
  border: 1px solid #e4e7ec;
  border-radius: 7px;
}

.sync-dot {
  width: 5px;
  height: 5px;
  background: #12b76a;
  border-radius: 50%;
  box-shadow: 0 0 0 3px rgba(18, 183, 106, 0.1);
}

.sync-state.busy .sync-dot {
  background: #14b8a6;
  box-shadow: 0 0 0 3px rgba(20, 184, 166, 0.12);
  animation: blink 1.2s ease-in-out infinite;
}

.version-chip {
  display: flex;
  gap: 5px;
  align-items: center;
  height: 27px;
  padding: 0 8px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  font-weight: 600;
  color: #667085;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.6);
  border: 1px solid #e4e7ec;
  border-radius: 7px;
  transition:
    color 150ms ease,
    background 150ms ease,
    border-color 150ms ease;
}

.version-chip:hover:not(:disabled) {
  color: #0f766e;
  background: #fff;
  border-color: #a7e5da;
}

.version-chip.available {
  color: #0f766e;
  background: #ecfdf9;
  border-color: #bdf3e8;
}

.version-chip:disabled {
  cursor: wait;
}

.version-chip .el-icon {
  font-size: 11px;
}

.version-dot {
  width: 4px;
  height: 4px;
  background: #0f766e;
  border-radius: 50%;
  box-shadow: 0 0 0 2px rgba(15, 118, 110, 0.12);
}

.spinning {
  animation: spin 1s linear infinite;
}

.header-button {
  height: 32px;
  padding: 0 11px;
  font-size: 10.5px;
  border-radius: 8px;
}

.add-folder-button {
  color: #475467;
  background: rgba(255, 255, 255, 0.76);
  border-color: #e4e7ec;
}

.add-folder-button:hover:not(:disabled) {
  color: #0f766e;
  background: #fff;
  border-color: #a7e5da;
}

.scan-button {
  box-shadow: 0 7px 18px rgba(15, 118, 110, 0.18);
}

.hero {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding: 25px 2px 23px;
}

.hero-copy {
  max-width: 690px;
}

.eyebrow {
  display: flex;
  gap: 7px;
  align-items: center;
  margin-bottom: 10px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 650;
  letter-spacing: 2.1px;
  color: #0f766e;
}

.eyebrow i {
  width: 16px;
  height: 1px;
  background: #14b8a6;
}

.hero h1 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: clamp(30px, 3.1vw, 38px);
  font-weight: 650;
  line-height: 1.2;
  color: #101828;
  letter-spacing: -1.4px;
  text-wrap: balance;
}

.hero h1 span {
  color: #0f766e;
}

.hero p {
  max-width: 640px;
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.8;
  color: #667085;
}

.hero-safety {
  display: flex;
  flex-direction: column;
  gap: 7px;
  align-items: flex-end;
  padding-bottom: 4px;
  font-size: 9.5px;
  color: #98a2b3;
  white-space: nowrap;
}

.hero-safety span {
  display: flex;
  gap: 6px;
  align-items: center;
}

.hero-safety i {
  width: 3px;
  height: 3px;
  background: #14b8a6;
  border-radius: 50%;
}

.update-slot {
  margin: -7px 0 14px;
}

.targets-section {
  margin-top: 23px;
}

.section-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding: 0 3px 10px;
}

.section-title {
  display: flex;
  gap: 9px;
  align-items: baseline;
}

.section-kicker {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 650;
  letter-spacing: 1.7px;
  color: #8fb8b1;
}

.section-title h2 {
  display: flex;
  gap: 7px;
  align-items: center;
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  font-weight: 650;
  letter-spacing: -0.1px;
  color: #344054;
}

.title-count {
  display: inline-grid;
  min-width: 19px;
  height: 17px;
  padding: 0 5px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 5px;
  place-items: center;
}

.sort-caption {
  font-size: 9px;
  color: #98a2b3;
}

.app-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 15px 3px 2px;
  font-size: 8.5px;
  color: #b0b8c4;
}

.footer-brand {
  letter-spacing: 0.15px;
}

.footer-note {
  display: flex;
  gap: 6px;
  align-items: center;
}

.footer-note i {
  width: 3px;
  height: 3px;
  background: #b0b8c4;
  border-radius: 50%;
}

@keyframes blink {
  0%,
  100% {
    opacity: 1;
  }
  50% {
    opacity: 0.35;
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1120px) {
  .main-content {
    padding-right: 22px;
    padding-left: 22px;
  }

  .hero-safety {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .sync-state.busy .sync-dot,
  .spinning {
    animation: none;
  }
}
</style>

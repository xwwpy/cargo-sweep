<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import { operationBusy as busy } from "../composables/useOperationBusy";
import {
  CircleCheck,
  Clock,
  CopyDocument,
  Cpu,
  Delete,
  Download,
  InfoFilled,
  Refresh,
  Switch,
  WarningFilled,
} from "@element-plus/icons-vue";
import {
  checkRustToolchainLatest,
  detectRustToolchain,
  errorMessage,
  getCargoHomeStatus,
  getRustupSourceStatus,
  applyRustupSource,
  installRustup,
  installRustToolchain,
  migrateCargoHome,
  migrateRustupHome,
  pickCargoHomeFolder,
  pickRustupHomeFolder,
  setDefaultRustToolchain,
  setHomePath,
  setRustupPath,
  uninstallRustToolchain,
  updateRustToolchain,
} from "../api";
import type {
  CargoHomeStatus,
  InstalledRustToolchain,
  LatestRustToolchain,
  LatestRustVersions,
  RustChannel,
  RustToolchainStatus,
  RustupSourceStatus,
} from "../types";

interface ChannelView {
  key: "stable" | "nightly";
  label: string;
  installed: boolean;
  toolchain: string | null;
  currentRelease: string | null;
  currentFull: string | null;
  currentBuild: string | null;
  latest: LatestRustToolchain | null;
  latestRelease: string | null;
  latestBuild: string | null;
  updateAvailable: boolean | null;
  active: boolean;
  default: boolean;
}

const emit = defineEmits<{
  "rust-state": [installed: boolean];
}>();

const status = ref<RustToolchainStatus | null>(null);
const latest = ref<LatestRustVersions | null>(null);
const latestError = ref<string | null>(null);
const loading = ref(true);
const latestLoading = ref(true);
const error = ref<string | null>(null);
const actionOutput = ref<string | null>(null);
const actionSuccess = ref<boolean | null>(null);
const elapsed = ref(0);
const historyTarget = ref("");
const customTarget = ref("");
const rustupPathInput = ref("");
const cargoHome = ref<CargoHomeStatus | null>(null);
const cargoHomeError = ref<string | null>(null);
const rustupSource = ref<RustupSourceStatus | null>(null);
const rustupSourceError = ref<string | null>(null);
const selectedSourceId = ref("official");
const customDistServer = ref("");
const customUpdateRoot = ref("");
const cargoHomeDialog = ref(false);
const cargoHomeTarget = ref("");
const createCargoLink = ref(true);
const cargoMigrating = ref(false);
const migrationError = ref<string | null>(null);
const rustupHomeDialog = ref(false);
const rustupHomeTarget = ref("");
const createRustupLink = ref(true);
const rustupMigrating = ref(false);
const rustupMigrationError = ref<string | null>(null);
const isWindows = /Windows/i.test(navigator.userAgent);
let timer: number | undefined;

interface CommandEntry {
  label: string;
  command: string;
}

const rustupReady = computed(() => status.value?.rustupInstalled ?? false);
const channelViews = computed<ChannelView[]>(() => {
  const result: ChannelView[] = [];
  for (const key of ["stable", "nightly"] as const) {
    const info = status.value?.channels.find((channel) => channel.channel === key);
    const latestInfo =
      key === "stable" ? latest.value?.stable ?? null : latest.value?.nightly ?? null;
    result.push(channelView(key, info, latestInfo));
  }
  return result;
});
const currentChannel = computed(
  () =>
    channelViews.value.find((channel) => channel.default) ??
    channelViews.value.find((channel) => channel.active) ??
    null,
);
const activeLatest = computed(() => currentChannel.value?.latest ?? null);
const currentVersion = computed(() => safeVersion(status.value?.version));
const latestVersion = computed(() =>
  safeVersion(activeLatest.value?.release),
);
const activeToolchainName = computed(() =>
  safeToolchainName(
    status.value?.activeToolchain ?? status.value?.defaultToolchain,
  ),
);
const currentBuild = computed(() => currentChannel.value?.currentBuild ?? null);
const latestBuild = computed(() => currentChannel.value?.latestBuild ?? null);
const currentIsLatest = computed(() => {
  const current = currentChannel.value;
  if (!current?.installed || !current.latest) {
    return null;
  }
  return sameVersion(current.currentFull, `rustc ${current.latest.version}`);
});
const currentIsNewBuild = computed(
  () =>
    currentIsLatest.value === false &&
    currentChannel.value?.currentRelease === currentChannel.value?.latestRelease,
);
const historyOptions = computed(() => latest.value?.historicalVersions ?? []);
const installedToolchains = computed(() => status.value?.toolchains ?? []);
const busyLabel = computed(() => {
  const value = busy.value;
  if (!value) {
    return "";
  }
  if (value === "rustup") {
    return "正在下载并安装 rustup";
  }
  if (value === "rustup-path") {
    return "正在保存 rustup 路径";
  }
  if (value.startsWith("install-")) {
    return `正在下载并安装 ${toolchainLabel(value.slice("install-".length))}`;
  }
  if (value.startsWith("update-")) {
    return `正在更新 ${toolchainLabel(value.slice("update-".length))}`;
  }
  if (value.startsWith("switch-")) {
    return `正在切换 ${toolchainLabel(value.slice("switch-".length))}`;
  }
  if (value.startsWith("uninstall-")) {
    return "正在删除工具链";
  }
  if (value === "migrate-cargo-home") {
    return "正在迁移 CARGO_HOME";
  }
  if (value === "migrate-rustup-home") {
    return "正在迁移 RUSTUP_HOME";
  }
  if (value === "configure-cargo") {
    return "正在更新 CARGO_HOME 配置";
  }
  if (value === "configure-rustup") {
    return "正在更新 RUSTUP_HOME 配置";
  }
  if (value === "apply-rustup-source") {
    return "正在切换 Rustup 下载源";
  }
  return "正在处理，请稍候";
});
const progressHint = computed(() =>
  busy.value ? "操作进行中，请保持应用开启；完成前应用不会关闭。" : "",
);
function toolchainLabel(value: string): string {
  if (value === "stable") {
    return "Stable";
  }
  if (value === "nightly") {
    return "Nightly";
  }
  return value;
}
const environmentRows = computed<
  {
    name: string;
    value: string;
    note: string;
    setKind: "cargo" | "rustup" | null;
  }[]
>(() => {
  if (!cargoHome.value) {
    return [];
  }
  return [
    {
      name: "PATH",
      value: cargoHome.value.cargoBin,
      note: cargoHome.value.pathContainsCargoBin ? "已包含" : "安装/迁移后追加",
      setKind: null,
    },
    {
      name: "CARGO_HOME",
      value: cargoHome.value.current,
      note:
        cargoHome.value.source === "environment"
          ? "用户环境变量"
          : cargoHome.value.source === "app"
            ? "应用配置"
            : "默认路径",
      setKind: cargoHome.value.source === "default" ? ("cargo" as const) : null,
    },
    {
      name: "RUSTUP_HOME",
      value: cargoHome.value.rustup.current,
      note:
        cargoHome.value.rustup.source === "environment"
          ? "用户环境变量"
          : cargoHome.value.rustup.source === "app"
            ? "应用配置"
            : "默认路径",
      setKind:
        cargoHome.value.rustup.source === "default" ? ("rustup" as const) : null,
    },
  ];
});
const linkLabel = computed(() => {
  if (!cargoHome.value?.linkExists) {
    return "CARGO_HOME 未链接";
  }
  return cargoHome.value.linkMatches
    ? "CARGO_HOME 已链接"
    : "CARGO_HOME 链接异常";
});
const rustupLinkLabel = computed(() => {
  if (!cargoHome.value?.rustup.linkExists) {
    return "RUSTUP_HOME 未链接";
  }
  return cargoHome.value.rustup.linkMatches
    ? "RUSTUP_HOME 已链接"
    : "RUSTUP_HOME 链接异常";
});
const rustupInstallCommand = computed(() =>
  isWindows
    ? "winget install --id Rustlang.Rustup -e --silent --accept-package-agreements --accept-source-agreements"
    : "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y",
);
const selectedMirror = computed(() =>
  rustupSource.value?.mirrors.find(
    (mirror) => mirror.id === selectedSourceId.value,
  ),
);
const sourceDistServer = computed(() =>
  selectedSourceId.value === "custom"
    ? customDistServer.value.trim()
    : selectedMirror.value?.distServer ?? "",
);
const sourceUpdateRoot = computed(() =>
  selectedSourceId.value === "custom"
    ? customUpdateRoot.value.trim()
    : selectedMirror.value?.updateRoot ?? "",
);
const sourceCommand = computed(() =>
  buildSourceCommand(
    selectedSourceId.value,
    sourceDistServer.value,
    sourceUpdateRoot.value,
  ),
);
const sourceApplyDisabled = computed(
  () =>
    Boolean(busy.value) ||
    !rustupSource.value ||
    !sourceDistServer.value ||
    !sourceUpdateRoot.value,
);
const manualCommands = computed<CommandEntry[]>(() => {
  const entries: CommandEntry[] = [
    { label: "安装 rustup", command: rustupInstallCommand.value },
    {
      label: "安装最新 Stable",
      command: installToolchainCommand("stable"),
    },
    {
      label: "安装最新 Nightly",
      command: installToolchainCommand("nightly"),
    },
    {
      label: "安装历史版本（替换版本号）",
      command: installToolchainCommand("1.85.0"),
    },
    { label: "更新全部工具链", command: "rustup update" },
  ];
  for (const toolchain of status.value?.toolchains ?? []) {
    if (!toolchain.default && !toolchain.active) {
      entries.push({
        label: `设为默认：${toolchain.name}`,
        command: defaultToolchainCommand(toolchain.name),
      });
      entries.push({
        label: `删除：${toolchain.name}`,
        command: uninstallToolchainCommand(toolchain.name),
      });
    }
  }
  if (cargoHome.value) {
    entries.push({
      label: "设置 CARGO_HOME（替换为实际路径）",
      command: envCommand("CARGO_HOME", cargoHome.value.current),
    });
    entries.push({
      label: "设置 RUSTUP_HOME（替换为实际路径）",
      command: envCommand("RUSTUP_HOME", cargoHome.value.rustup.current),
    });
  }
  return entries;
});
const elapsedLabel = computed(() => {
  const minutes = Math.floor(elapsed.value / 60);
  const seconds = elapsed.value % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
});

function channelView(
  key: "stable" | "nightly",
  info: RustChannel | undefined,
  latestInfo: LatestRustToolchain | null,
): ChannelView {
  const currentFull = info?.rustcVersion ?? null;
  return {
    key,
    label: key === "stable" ? "Stable" : "Nightly",
    installed: Boolean(info?.installed),
    toolchain: info?.toolchain ?? null,
    currentRelease: safeVersion(info?.release),
    currentFull,
    currentBuild: buildLabel(currentFull, null),
    latest: latestInfo,
    latestRelease: safeVersion(latestInfo?.release),
    latestBuild: latestInfo
      ? buildLabel(`rustc ${latestInfo.version}`, latestInfo.date)
      : null,
    updateAvailable:
      info?.installed && latestInfo && currentFull
        ? !sameVersion(currentFull, `rustc ${latestInfo.version}`)
        : null,
    active: Boolean(info?.active),
    default: Boolean(info?.default),
  };
}

function safeVersion(value: string | null | undefined): string | null {
  if (!value) {
    return null;
  }
  const trimmed = value.trim();
  if (!/\d/.test(trimmed)) {
    return null;
  }
  return trimmed;
}

function safeToolchainName(value: string | null | undefined): string | null {
  if (!value || value.trim().toLowerCase() === "the") {
    return null;
  }
  return value;
}

function quoteShell(value: string): string {
  if (isWindows) {
    return `"${value.replace(/"/g, '\\"')}"`;
  }
  return `'${value.replace(/'/g, `'\\''`)}'`;
}

function installToolchainCommand(target: string): string {
  return `rustup toolchain install ${target} --profile default`;
}

function updateToolchainCommand(toolchain: string): string {
  return `rustup update ${toolchain} --no-self-update`;
}

function defaultToolchainCommand(toolchain: string): string {
  return `rustup default ${toolchain}`;
}

function uninstallToolchainCommand(toolchain: string): string {
  return `rustup toolchain uninstall ${toolchain}`;
}

function envCommand(variable: string, value: string): string {
  return isWindows
    ? `setx ${variable} ${quoteShell(value)}`
    : `export ${variable}=${quoteShell(value)}`;
}

function channelPrimaryCommand(view: ChannelView): string {
  if (!view.installed) {
    return installToolchainCommand(view.key);
  }
  return updateToolchainCommand(view.toolchain ?? view.key);
}

function channelDefaultCommand(view: ChannelView): string | null {
  if (!view.installed || view.default || !view.toolchain) {
    return null;
  }
  return defaultToolchainCommand(view.toolchain);
}

function migrationCommands(
  kind: "cargo" | "rustup",
  current: string | null,
  target: string,
  createLink: boolean,
): string {
  if (!current || !target) {
    return "";
  }
  const variable = kind === "cargo" ? "CARGO_HOME" : "RUSTUP_HOME";
  const backup = `${current}.cargo-sweep-backup`;
  if (isWindows) {
    const lines = [
      `robocopy ${quoteShell(current)} ${quoteShell(target)} /E`,
      `setx ${variable} ${quoteShell(target)}`,
    ];
    if (createLink) {
      const backupName = backup.split(/[\\/]/).pop() ?? backup;
      lines.splice(
        1,
        0,
        `ren ${quoteShell(current)} ${quoteShell(backupName)}`,
      );
      lines.push(`mklink /J ${quoteShell(current)} ${quoteShell(target)}`);
    }
    return lines.join("\n");
  }
  const lines = [
    `cp -a ${quoteShell(`${current}/.`)} ${quoteShell(`${target}/`)}`,
    `export ${variable}=${quoteShell(target)}`,
  ];
  if (createLink) {
    lines.splice(
      1,
      0,
      `mv ${quoteShell(current)} ${quoteShell(backup)}`,
      `ln -s ${quoteShell(target)} ${quoteShell(current)}`,
    );
  }
  return `# 将 export 行加入 ~/.zprofile 或 ~/.profile\n${lines.join("\n")}`;
}

const cargoMigrationCommand = computed(() =>
  migrationCommands(
    "cargo",
    cargoHome.value?.current ?? null,
    cargoHomeTarget.value,
    createCargoLink.value,
  ),
);
const rustupMigrationCommand = computed(() =>
  migrationCommands(
    "rustup",
    cargoHome.value?.rustup.current ?? null,
    rustupHomeTarget.value,
    createRustupLink.value,
  ),
);

async function copyText(value: string) {
  if (!value) {
    return;
  }
  try {
    await navigator.clipboard.writeText(value);
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = value;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
  ElMessage.success("命令已复制");
}

function sameVersion(left: string | null, right: string | null): boolean {
  if (!left || !right) {
    return false;
  }
  return versionIdentity(left) === versionIdentity(right);
}

function versionIdentity(value: string): string {
  const build = value.match(/\(([0-9a-f]+)\s+(\d{4}-\d{2}-\d{2})\)/i);
  if (build) {
    return `${build[1].toLowerCase()} ${build[2]}`;
  }
  return value.replace(/^rustc\s+/, "").trim();
}

function buildLabel(
  fullVersion: string | null,
  fallbackDate: string | null,
): string | null {
  if (fullVersion) {
    const match = fullVersion.match(
      /\(([0-9a-f]+)(?:\s+(\d{4}-\d{2}-\d{2}))?\)/i,
    );
    if (match) {
      const hash = match[1].slice(0, 7);
      return match[2] ? `${match[2]} · ${hash}` : hash;
    }
    const date = fullVersion.match(/\d{4}-\d{2}-\d{2}/);
    if (date) {
      return date[0];
    }
  }
  return fallbackDate;
}

function channelState(view: ChannelView): string {
  if (!view.installed) {
    return view.latest ? `最新 ${view.latestRelease}` : "未安装";
  }
  if (latestLoading.value) {
    return "检查最新版本…";
  }
  if (!view.latest) {
    return "最新版本未知";
  }
  if (view.updateAvailable === false) {
    return "已是最新";
  }
  if (view.currentRelease === view.latestRelease) {
    return `有新构建${view.latestBuild ? ` · ${view.latestBuild}` : ""}`;
  }
  return `可更新到 ${view.latestRelease}`;
}

function actionLabel(view: ChannelView): string {
  if (!view.installed) {
    return "安装";
  }
  if (latestLoading.value) {
    return "检测中";
  }
  if (!view.latest) {
    return "更新";
  }
  return view.updateAvailable ? "更新" : "已是最新";
}

function canUseAction(view: ChannelView): boolean {
  if (!rustupReady.value || busy.value) {
    return false;
  }
  if (!view.installed) {
    return true;
  }
  if (latestLoading.value) {
    return false;
  }
  return view.updateAvailable === true;
}

function startTimer() {
  elapsed.value = 0;
  window.clearInterval(timer);
  timer = window.setInterval(() => {
    elapsed.value += 1;
  }, 1000);
}

function stopTimer() {
  window.clearInterval(timer);
  timer = undefined;
}

async function loadLatest() {
  latestLoading.value = true;
  latestError.value = null;
  try {
    latest.value = await checkRustToolchainLatest();
    latestError.value = latest.value.error;
  } catch (reason) {
    latestError.value = errorMessage(reason);
  } finally {
    latestLoading.value = false;
  }
}

async function loadCargoHome() {
  cargoHomeError.value = null;
  try {
    cargoHome.value = await getCargoHomeStatus();
  } catch (reason) {
    cargoHomeError.value = errorMessage(reason);
  }
}

async function loadRustupSource() {
  rustupSourceError.value = null;
  try {
    const next = await getRustupSourceStatus();
    rustupSource.value = next;
    selectedSourceId.value = next.currentId;
    customDistServer.value = next.distServer;
    customUpdateRoot.value = next.updateRoot;
  } catch (reason) {
    rustupSourceError.value = errorMessage(reason);
  }
}

function selectSource(sourceId: string) {
  selectedSourceId.value = sourceId;
  if (sourceId === "custom" && rustupSource.value) {
    customDistServer.value = rustupSource.value.distServer;
    customUpdateRoot.value = rustupSource.value.updateRoot;
  }
}

function buildSourceCommand(
  sourceId: string,
  distServer: string,
  updateRoot: string,
): string {
  if (sourceId === "official") {
    return isWindows
      ? [
          'reg delete "HKCU\\Environment" /v RUSTUP_DIST_SERVER /f',
          'reg delete "HKCU\\Environment" /v RUSTUP_UPDATE_ROOT /f',
        ].join("\n")
      : [
          "unset RUSTUP_DIST_SERVER RUSTUP_UPDATE_ROOT",
          "# 删除 ~/.zprofile 或 ~/.profile 中由 Cargo Sweep 写入的 RUSTUP_DIST_SERVER/RUSTUP_UPDATE_ROOT 块",
        ].join("\n");
  }
  return isWindows
    ? [
        `setx RUSTUP_DIST_SERVER ${quoteShell(distServer)}`,
        `setx RUSTUP_UPDATE_ROOT ${quoteShell(updateRoot)}`,
      ].join("\n")
    : [
        "# 将以下 export 加入 ~/.zprofile 或 ~/.profile",
        `export RUSTUP_DIST_SERVER=${quoteShell(distServer)}`,
        `export RUSTUP_UPDATE_ROOT=${quoteShell(updateRoot)}`,
      ].join("\n");
}

async function applySource() {
  if (sourceApplyDisabled.value) {
    return;
  }
  busy.value = "apply-rustup-source";
  actionOutput.value = null;
  actionSuccess.value = null;
  startTimer();
  try {
    const status = await applyRustupSource(
      selectedSourceId.value,
      selectedSourceId.value === "custom" ? sourceDistServer.value : null,
      selectedSourceId.value === "custom" ? sourceUpdateRoot.value : null,
    );
    rustupSource.value = status;
    selectedSourceId.value = status.currentId;
    customDistServer.value = status.distServer;
    customUpdateRoot.value = status.updateRoot;
    actionOutput.value = `Rustup 下载源已切换为 ${status.currentName}`;
    actionSuccess.value = true;
    ElMessage.success("Rustup 下载源已更新");
  } catch (reason) {
    actionOutput.value = errorMessage(reason);
    actionSuccess.value = false;
    ElMessage.error("切换下载源失败，请查看日志");
  } finally {
    stopTimer();
    busy.value = null;
  }
}

async function load() {
  loading.value = true;
  error.value = null;
  void loadCargoHome();
  void loadRustupSource();
  try {
    status.value = await detectRustToolchain();
    rustupPathInput.value =
      status.value.rustupCustomPath ?? status.value.rustupPath ?? "";
    emit("rust-state", status.value.installed);
    void loadLatest();
  } catch (reason) {
    error.value = errorMessage(reason);
  } finally {
    loading.value = false;
  }
}

async function runOperation(
  label: string,
  operation: () => Promise<{ success: boolean; output: string; status: RustToolchainStatus }>,
  successMessage: string,
) {
  if (busy.value) {
    return;
  }
  busy.value = label;
  actionOutput.value = null;
  actionSuccess.value = null;
  startTimer();
  try {
    const result = await operation();
    status.value = result.status;
    actionOutput.value = result.output;
    actionSuccess.value = result.success;
    emit("rust-state", result.status.installed);
    if (result.success) {
      ElMessage.success(successMessage);
    } else {
      ElMessage.error("操作未完成，请查看日志");
    }
  } catch (reason) {
    actionSuccess.value = false;
    actionOutput.value = errorMessage(reason);
    ElMessage.error("操作失败，请查看日志");
  } finally {
    stopTimer();
    busy.value = null;
  }
}

async function saveRustupPath() {
  if (!rustupPathInput.value.trim()) {
    ElMessage.warning("请输入 rustup 可执行文件或所在目录");
    return;
  }
  await runOperation(
    "rustup-path",
    async () => {
      const next = await setRustupPath(rustupPathInput.value);
      rustupPathInput.value = next.rustupCustomPath ?? next.rustupPath ?? "";
      return {
        success: true,
        output: `已保存 rustup 路径：${next.rustupPath ?? rustupPathInput.value}`,
        status: next,
      };
    },
    "rustup 路径已保存",
  );
}

async function installRustupAction() {
  await runOperation("rustup", installRustup, "rustup 已安装");
}

function openCargoHomeDialog() {
  if (!cargoHome.value || cargoHome.value.linkExists || busy.value) {
    return;
  }
  cargoHomeTarget.value = "";
  createCargoLink.value = true;
  migrationError.value = null;
  cargoHomeDialog.value = true;
}

async function pickCargoHomeTarget() {
  const selected = await pickCargoHomeFolder();
  if (selected) {
    cargoHomeTarget.value = selected;
  }
}

async function runCargoHomeMigration() {
  if (!cargoHomeTarget.value.trim()) {
    ElMessage.warning("请选择新的 CARGO_HOME 目录");
    return;
  }
  if (busy.value) {
    return;
  }
  busy.value = "migrate-cargo-home";
  cargoMigrating.value = true;
  migrationError.value = null;
  actionOutput.value = null;
  actionSuccess.value = null;
  startTimer();
  try {
    const result = await migrateCargoHome(
      cargoHomeTarget.value.trim(),
      createCargoLink.value,
    );
    cargoHome.value = result.status;
    actionOutput.value = result.output;
    actionSuccess.value = true;
    cargoHomeDialog.value = false;
    ElMessage.success("CARGO_HOME 已迁移");
    await load();
  } catch (reason) {
    migrationError.value = errorMessage(reason);
    actionOutput.value = migrationError.value;
    actionSuccess.value = false;
    ElMessage.error("迁移失败，请查看日志");
  } finally {
    stopTimer();
    busy.value = null;
    cargoMigrating.value = false;
  }
}

function openRustupHomeDialog() {
  if (!cargoHome.value || cargoHome.value.rustup.linkExists || busy.value) {
    return;
  }
  rustupHomeTarget.value = "";
  createRustupLink.value = true;
  rustupMigrationError.value = null;
  rustupHomeDialog.value = true;
}

async function pickRustupHomeTarget() {
  const selected = await pickRustupHomeFolder();
  if (selected) {
    rustupHomeTarget.value = selected;
  }
}

async function runRustupHomeMigration() {
  if (!rustupHomeTarget.value.trim()) {
    ElMessage.warning("请选择新的 RUSTUP_HOME 目录");
    return;
  }
  if (busy.value) {
    return;
  }
  busy.value = "migrate-rustup-home";
  rustupMigrating.value = true;
  rustupMigrationError.value = null;
  actionOutput.value = null;
  actionSuccess.value = null;
  startTimer();
  try {
    const result = await migrateRustupHome(
      rustupHomeTarget.value.trim(),
      createRustupLink.value,
    );
    cargoHome.value = result.status;
    actionOutput.value = result.output;
    actionSuccess.value = true;
    rustupHomeDialog.value = false;
    ElMessage.success("RUSTUP_HOME 已迁移");
    await load();
  } catch (reason) {
    rustupMigrationError.value = errorMessage(reason);
    actionOutput.value = rustupMigrationError.value;
    actionSuccess.value = false;
    ElMessage.error("迁移失败，请查看日志");
  } finally {
    stopTimer();
    busy.value = null;
    rustupMigrating.value = false;
  }
}

async function chooseHomeLocation(kind: "cargo" | "rustup") {
  const selected =
    kind === "cargo"
      ? await pickCargoHomeFolder()
      : await pickRustupHomeFolder();
  if (!selected) {
    return;
  }
  const label = kind === "cargo" ? "CARGO_HOME" : "RUSTUP_HOME";
  try {
    await ElMessageBox.confirm(
      `将 ${label} 设置为 ${selected}？\n\n仅更新环境配置，不会自动复制现有数据。旧目录数据会保留，可稍后通过“更改 ${label}”迁移。`,
      `设置 ${label}`,
      {
        type: "warning",
        confirmButtonText: "保存配置",
        cancelButtonText: "取消",
      },
    );
  } catch {
    return;
  }
  if (busy.value) {
    return;
  }
  busy.value = `configure-${kind}`;
  actionOutput.value = null;
  actionSuccess.value = null;
  startTimer();
  try {
    const next = await setHomePath(kind, selected);
    cargoHome.value = next;
    actionOutput.value = `已更新 ${label} 配置：${selected}`;
    actionSuccess.value = true;
    ElMessage.success(`${label} 配置已更新`);
    await load();
  } catch (reason) {
    actionOutput.value = errorMessage(reason);
    actionSuccess.value = false;
    ElMessage.error("更新配置失败，请查看日志");
  } finally {
    stopTimer();
    busy.value = null;
  }
}

async function removeToolchain(toolchain: InstalledRustToolchain) {
  if (toolchain.default || toolchain.active) {
    ElMessage.warning("默认工具链不能直接删除，请先切换到其他工具链");
    return;
  }
  try {
    await ElMessageBox.confirm(
      `删除工具链 ${toolchain.name}？此操作会移除对应的 Rust 工具链文件。`,
      "删除工具链",
      {
        type: "warning",
        confirmButtonText: "删除",
        cancelButtonText: "取消",
      },
    );
  } catch {
    return;
  }
  await runOperation(
    `uninstall-${toolchain.name}`,
    () => uninstallRustToolchain(toolchain.name),
    "工具链已删除",
  );
}

async function installChannel(view: ChannelView) {
  const setDefault = !status.value?.installed || !status.value?.defaultToolchain;
  await runOperation(
    `install-${view.key}`,
    () => installRustToolchain(view.key, setDefault),
    `${view.label} 工具链已安装`,
  );
}

async function updateChannel(view: ChannelView) {
  await runOperation(
    `update-${view.key}`,
    () => updateRustToolchain(view.toolchain ?? view.key),
    `${view.label} 工具链已更新`,
  );
}

async function switchChannel(view: ChannelView) {
  if (!view.installed || !view.toolchain) {
    return;
  }
  await runOperation(
    `switch-${view.key}`,
    () => setDefaultRustToolchain(view.toolchain as string),
    `默认工具链已切换为 ${view.label}`,
  );
}

async function switchToolchain(name: string) {
  if (busy.value) {
    return;
  }
  await runOperation(
    `switch-${name}`,
    () => setDefaultRustToolchain(name),
    "默认工具链已切换",
  );
}

async function installHistory() {
  if (!historyTarget.value) {
    ElMessage.warning("请选择要安装的历史版本");
    return;
  }
  await runOperation(
    `install-${historyTarget.value}`,
    () => installRustToolchain(historyTarget.value, false),
    `${historyTarget.value} 工具链已安装`,
  );
}

async function installCustom() {
  const target = customTarget.value.trim();
  if (!/^(stable|beta|nightly|\d+\.\d+(\.\d+)?)$/.test(target)) {
    ElMessage.warning("请输入 stable、beta、nightly 或类似 1.85.0 的版本号");
    return;
  }
  await runOperation(
    `install-${target}`,
    () => installRustToolchain(target, false),
    `${target} 工具链已安装`,
  );
}

function retryLatest() {
  void loadLatest();
}

onMounted(() => {
  void load();
});

onUnmounted(stopTimer);
</script>

<template>
  <section class="toolchain-page">
    <header class="toolchain-hero">
      <div>
        <span class="eyebrow"><i /> RUST TOOLCHAIN</span>
        <h1>工具链，<span>版本一目了然。</span></h1>
        <p>查看当前版本与最新版本，按需安装、更新或切换。</p>
      </div>
      <el-button class="refresh-button" :loading="loading || latestLoading" @click="load">
        <el-icon v-if="!loading && !latestLoading"><Refresh /></el-icon>
        重新检测
      </el-button>
    </header>

    <div v-if="error" class="toolchain-error">
      <div class="error-icon"><el-icon><WarningFilled /></el-icon></div>
      <div>
        <h3>无法读取 Rust 工具链</h3>
        <p>{{ error }}</p>
      </div>
      <el-button class="retry-button" @click="load">重试</el-button>
    </div>

    <div v-else-if="loading && !status" class="toolchain-loading">
      <div class="loading-mark"><el-icon><Cpu /></el-icon></div>
      <h3>正在读取工具链信息</h3>
      <p>检查 rustc、rustup 与已安装工具链</p>
    </div>

    <template v-else-if="status">
      <div v-if="busy" class="operation-progress">
        <div class="operation-progress-head">
          <span>
            <el-icon class="spinning"><Clock /></el-icon>
            {{ busyLabel }}
          </span>
          <span>{{ elapsedLabel }}</span>
        </div>
        <el-progress
          :percentage="100"
          :show-text="false"
          :indeterminate="true"
          :duration="1.8"
          :stroke-width="5"
          color="#0f766e"
        />
        <p>{{ progressHint }}</p>
      </div>

      <div v-if="!status.installed" class="setup-banner">
        <div class="setup-icon"><el-icon><InfoFilled /></el-icon></div>
        <div class="setup-copy">
          <h2>
            {{
              status.toolchains.length > 0
                ? "已安装工具链，但还没有默认工具链"
                : "还没有安装 Rust 工具链"
            }}
          </h2>
          <p>
            {{
              status.toolchains.length > 0
                ? "在下方选择 Stable 或 Nightly 并设为默认即可。"
                : "安装 Stable 或 Nightly 后即可使用 Cargo 下载源管理。"
            }}
          </p>
        </div>
        <div class="setup-versions">
          <span v-if="latest?.stable">Stable {{ safeVersion(latest.stable.release) || "未知" }}</span>
          <span v-if="latest?.nightly">Nightly {{ safeVersion(latest.nightly.release) || "未知" }}</span>
        </div>
      </div>

      <div v-if="!status.rustupInstalled" class="rustup-banner">
        <div class="rustup-copy">
          <h2>未检测到 rustup</h2>
          <p>已安装但未自动识别？手动指定 rustup 路径，或一键安装。</p>
        </div>
        <div class="rustup-actions">
          <el-input
            v-model="rustupPathInput"
            class="rustup-path-input"
            placeholder="~/.cargo/bin/rustup"
            :disabled="Boolean(busy)"
            @keyup.enter="saveRustupPath"
          />
          <el-button
            :disabled="Boolean(busy) || !rustupPathInput"
            :loading="busy === 'rustup-path'"
            @click="saveRustupPath"
          >
            保存路径
          </el-button>
          <el-button
            type="primary"
            :loading="busy === 'rustup'"
            :disabled="Boolean(busy)"
            @click="installRustupAction"
          >
            <el-icon><Download /></el-icon>
            一键安装 rustup
          </el-button>
        </div>
        <div class="command-line">
          <code>{{ rustupInstallCommand }}</code>
          <button
            class="copy-button"
            type="button"
            title="复制安装命令"
            @click="copyText(rustupInstallCommand)"
          >
            <el-icon><CopyDocument /></el-icon>
          </button>
        </div>
      </div>

      <section v-if="status.installed" class="current-panel">
        <div class="current-mark"><el-icon><Cpu /></el-icon></div>
        <div class="current-copy">
          <span class="panel-label">当前默认工具链</span>
          <h2>{{ currentVersion || "未知版本" }}</h2>
          <code>{{ activeToolchainName || "未设置默认工具链" }}</code>
          <span v-if="currentBuild" class="current-build">{{ currentBuild }}</span>
        </div>
        <div class="current-latest">
          <span>最新版本</span>
          <strong>
            <template v-if="latestLoading">检测中…</template>
            <template v-else>{{ latestVersion || "未获取" }}</template>
          </strong>
          <small v-if="latestBuild">{{ latestBuild }}</small>
          <em v-if="currentIsLatest === true">已是最新</em>
          <em v-else-if="currentIsLatest === false" class="update-hint">
            {{ currentIsNewBuild ? "有新构建" : "有新版本" }}
          </em>
        </div>
      </section>

      <div v-if="latestError" class="latest-warning">
        <el-icon><WarningFilled /></el-icon>
        <span>最新版本获取失败：{{ latestError }}</span>
        <button type="button" @click="retryLatest">重试</button>
      </div>

      <section v-if="cargoHome" class="cargo-home-card">
        <div class="env-head">
          <div>
            <span class="section-kicker">ENVIRONMENT</span>
            <h2>环境变量与 Rust 存储</h2>
            <p class="env-sub">安装工具链 / rustup 时使用；PATH 只会在缺失时追加。</p>
          </div>
          <div class="link-badges">
            <span
              class="link-badge"
              :class="{
                ok: cargoHome.linkExists && cargoHome.linkMatches,
                warn: cargoHome.linkExists && !cargoHome.linkMatches,
              }"
            >
              {{ linkLabel }}
            </span>
            <span
              class="link-badge"
              :class="{
                ok: cargoHome.rustup.linkExists && cargoHome.rustup.linkMatches,
                warn: cargoHome.rustup.linkExists && !cargoHome.rustup.linkMatches,
              }"
            >
              {{ rustupLinkLabel }}
            </span>
          </div>
        </div>

        <div class="env-grid">
          <div v-for="row in environmentRows" :key="row.name" class="env-item">
            <div class="env-item-head">
              <code>{{ row.name }}</code>
              <span>{{ row.note }}</span>
            </div>
            <p>{{ row.value }}</p>
            <button
              v-if="row.setKind"
              class="env-configure"
              type="button"
              title="通过文件夹选择器更新配置，不迁移现有数据"
              @click="chooseHomeLocation(row.setKind)"
            >
              设置位置
            </button>
          </div>
        </div>

        <div v-if="cargoHome.linkExists" class="link-detail">
          <span>CARGO_HOME 链接</span>
          <code>{{ cargoHome.default }}</code>
          <span>→</span>
          <code>{{ cargoHome.linkTarget || "未知目标" }}</code>
        </div>

        <div v-if="cargoHome.rustup.linkExists" class="link-detail">
          <span>RUSTUP_HOME 链接</span>
          <code>{{ cargoHome.rustup.default }}</code>
          <span>→</span>
          <code>{{ cargoHome.rustup.linkTarget || "未知目标" }}</code>
        </div>

        <div class="cargo-home-footer">
          <p>
            更改位置会迁移对应数据；建议先关闭正在运行的 cargo、rustup 和 rustc 进程。
          </p>
          <div class="storage-actions">
            <el-button
              :disabled="cargoHome.linkExists || Boolean(busy)"
              :title="cargoHome.linkExists ? '已检测到旧路径链接，无需再次迁移' : ''"
              @click="openCargoHomeDialog"
            >
              更改 CARGO_HOME
            </el-button>
            <el-button
              :disabled="cargoHome.rustup.linkExists || Boolean(busy)"
              :title="cargoHome.rustup.linkExists ? '已检测到旧路径链接，无需再次迁移' : ''"
              @click="openRustupHomeDialog"
            >
              更改 RUSTUP_HOME
            </el-button>
          </div>
        </div>
      </section>

      <div v-else-if="cargoHomeError" class="cargo-home-error">
        <el-icon><WarningFilled /></el-icon>
        {{ cargoHomeError }}
      </div>

      <section v-if="rustupSource" class="rustup-source-card">
        <div class="source-head">
          <div>
            <span class="section-kicker">RUSTUP MIRROR</span>
            <h2>Rustup 下载源</h2>
            <p>影响 rustup 工具链和 rustup 自身的下载。</p>
          </div>
          <span class="source-current-badge">
            {{ rustupSource.currentName }}
          </span>
        </div>

        <div class="source-current">
          <span>当前配置</span>
          <code>{{ rustupSource.distServer }}</code>
          <code>{{ rustupSource.updateRoot }}</code>
          <small>{{ rustupSource.envConfigured ? "用户环境变量" : "默认官方源" }}</small>
        </div>

        <div class="source-options">
          <button
            v-for="mirror in rustupSource.mirrors"
            :key="mirror.id"
            type="button"
            class="source-option"
            :class="{ active: selectedSourceId === mirror.id }"
            :disabled="Boolean(busy)"
            @click="selectSource(mirror.id)"
          >
            {{ mirror.name }}
          </button>
          <button
            type="button"
            class="source-option"
            :class="{ active: selectedSourceId === 'custom' }"
            :disabled="Boolean(busy)"
            @click="selectSource('custom')"
          >
            自定义
          </button>
        </div>

        <div v-if="selectedSourceId === 'custom'" class="source-custom">
          <el-input
            v-model="customDistServer"
            placeholder="RUSTUP_DIST_SERVER，例如 https://rsproxy.cn"
            :disabled="Boolean(busy)"
          />
          <el-input
            v-model="customUpdateRoot"
            placeholder="RUSTUP_UPDATE_ROOT，例如 https://rsproxy.cn/rustup"
            :disabled="Boolean(busy)"
          />
        </div>

        <div class="source-preview">
          <div>
            <span>RUSTUP_DIST_SERVER</span>
            <code>{{ sourceDistServer || "未填写" }}</code>
          </div>
          <div>
            <span>RUSTUP_UPDATE_ROOT</span>
            <code>{{ sourceUpdateRoot || "未填写" }}</code>
          </div>
        </div>

        <div class="source-command">
          <div class="source-command-head">
            <span>等价命令</span>
            <button
              class="copy-button"
              type="button"
              title="复制下载源命令"
              @click="copyText(sourceCommand)"
            >
              <el-icon><CopyDocument /></el-icon>
            </button>
          </div>
          <pre>{{ sourceCommand }}</pre>
        </div>

        <div class="source-actions">
          <el-button
            type="primary"
            :loading="busy === 'apply-rustup-source'"
            :disabled="sourceApplyDisabled"
            @click="applySource"
          >
            应用下载源
          </el-button>
          <el-button :disabled="Boolean(busy)" @click="loadRustupSource">
            重新检测
          </el-button>
        </div>
        <p class="source-hint">
          写入用户环境变量；已打开的终端需要重新打开后才会使用新下载源。
        </p>
        <p v-if="rustupSource?.backupPath" class="source-backup-note">
          上次源配置已备份至 <code>{{ rustupSource?.backupPath }}</code>
        </p>
      </section>

      <div v-else-if="rustupSourceError" class="cargo-home-error">
        <el-icon><WarningFilled /></el-icon>
        {{ rustupSourceError }}
      </div>

      <div class="channel-grid">
        <article
          v-for="channel in channelViews"
          :key="channel.key"
          class="channel-card"
          :class="{ default: channel.default, installed: channel.installed }"
        >
          <div class="channel-head">
            <div>
              <span class="channel-name">{{ channel.label }}</span>
              <span class="channel-sub">工具链</span>
            </div>
            <span v-if="channel.default" class="status-badge active">
              <el-icon><CircleCheck /></el-icon>
              当前默认
            </span>
            <span v-else-if="channel.installed" class="status-badge">已安装</span>
            <span v-else class="status-badge muted">未安装</span>
          </div>

          <div class="version-pair">
            <div>
              <span class="version-label">当前</span>
              <strong>{{ channel.currentRelease || "—" }}</strong>
              <span v-if="channel.currentBuild" class="build-label">
                {{ channel.currentBuild }}
              </span>
            </div>
            <span class="version-arrow">→</span>
            <div>
              <span class="version-label">最新</span>
              <strong>
                <template v-if="latestLoading">检测中…</template>
                <template v-else>{{ channel.latestRelease || "未知" }}</template>
              </strong>
              <span v-if="channel.latestBuild" class="build-label">
                {{ channel.latestBuild }}
              </span>
            </div>
          </div>

          <p class="channel-state" :class="{ ok: channel.updateAvailable === false }">
            {{ channelState(channel) }}
          </p>

          <div class="channel-actions">
            <el-button
              v-if="!channel.installed"
              type="primary"
              :disabled="!rustupReady || Boolean(busy)"
              :loading="busy === `install-${channel.key}`"
              @click="installChannel(channel)"
            >
              <el-icon><Download /></el-icon>
              安装
            </el-button>
            <el-button
              v-else
              :type="channel.updateAvailable ? 'primary' : 'default'"
              :disabled="!canUseAction(channel)"
              :loading="busy === `update-${channel.key}`"
              @click="updateChannel(channel)"
            >
              <el-icon><Refresh /></el-icon>
              {{ actionLabel(channel) }}
            </el-button>
            <el-button
              v-if="channel.installed && !channel.default"
              class="switch-button"
              :disabled="Boolean(busy)"
              :loading="busy === `switch-${channel.key}`"
              @click="switchChannel(channel)"
            >
              <el-icon><Switch /></el-icon>
              设为默认
            </el-button>
          </div>

          <div class="card-commands">
            <div class="command-line">
              <span class="command-label">
                {{ channel.installed ? "更新命令" : "安装命令" }}
              </span>
              <code>{{ channelPrimaryCommand(channel) }}</code>
              <button
                class="copy-button"
                type="button"
                title="复制命令"
                @click="copyText(channelPrimaryCommand(channel))"
              >
                <el-icon><CopyDocument /></el-icon>
              </button>
            </div>
            <div v-if="channelDefaultCommand(channel)" class="command-line">
              <span class="command-label">切换命令</span>
              <code>{{ channelDefaultCommand(channel) }}</code>
              <button
                class="copy-button"
                type="button"
                title="复制命令"
                @click="copyText(channelDefaultCommand(channel) ?? '')"
              >
                <el-icon><CopyDocument /></el-icon>
              </button>
            </div>
          </div>
        </article>
      </div>

      <section class="history-card">
        <div class="history-head">
          <div>
            <span class="section-kicker">HISTORY</span>
            <h2>安装历史版本</h2>
          </div>
          <span v-if="!rustupReady" class="history-warning">需要先安装 rustup</span>
        </div>
        <div class="history-actions">
          <el-select
            v-model="historyTarget"
            placeholder="选择版本"
            :disabled="!rustupReady || Boolean(busy) || latestLoading"
            class="history-select"
          >
            <el-option
              v-for="version in historyOptions"
              :key="version"
              :label="version"
              :value="version"
            />
          </el-select>
          <el-button
            :disabled="!rustupReady || !historyTarget || Boolean(busy)"
            :loading="busy === `install-${historyTarget}`"
            @click="installHistory"
          >
            安装
          </el-button>
          <span class="history-divider" />
          <el-input
            v-model="customTarget"
            class="custom-version"
            placeholder="或输入 1.85.0"
            :disabled="!rustupReady || Boolean(busy)"
            @keyup.enter="installCustom"
          />
          <el-button
            :disabled="!rustupReady || !customTarget || Boolean(busy)"
            :loading="busy === `install-${customTarget.trim()}`"
            @click="installCustom"
          >
            安装自定义版本
          </el-button>
        </div>
      </section>

      <section v-if="installedToolchains.length > 0" class="installed-section">
        <div class="section-heading">
          <div>
            <span class="section-kicker">INSTALLED</span>
            <h2>已安装工具链</h2>
          </div>
          <span>{{ installedToolchains.length }} 个</span>
        </div>
        <div class="toolchain-list">
          <div
            v-for="toolchain in installedToolchains"
            :key="toolchain.name"
            class="toolchain-row"
            :class="{ default: toolchain.default }"
          >
            <span class="toolchain-dot" />
            <span class="toolchain-name">{{ toolchain.name }}</span>
            <span class="toolchain-version">{{ safeVersion(toolchain.release) || "未知" }}</span>
            <span v-if="toolchain.default" class="active-label">
              <el-icon><CircleCheck /></el-icon>
              默认
            </span>
            <el-button
              v-else
              class="row-switch"
              text
              :disabled="Boolean(busy)"
              @click="switchToolchain(toolchain.name)"
            >
              设为默认
            </el-button>
            <el-button
              v-if="!toolchain.default && !toolchain.active"
              class="row-delete"
              text
              :disabled="Boolean(busy)"
              title="删除工具链"
              @click="removeToolchain(toolchain)"
            >
              <el-icon><Delete /></el-icon>
            </el-button>
            <el-button
              v-else
              class="row-delete"
              text
              disabled
              title="默认或当前工具链不能删除，请先切换到其他工具链"
            >
              <el-icon><Delete /></el-icon>
            </el-button>
          </div>
        </div>
      </section>

      <section v-if="manualCommands.length > 0" class="manual-commands">
        <div class="manual-commands-head">
          <div>
            <span class="section-kicker">COMMANDS</span>
            <h2>手动执行命令</h2>
          </div>
          <span>可复制后在终端执行</span>
        </div>
        <div class="manual-command-list">
          <div
            v-for="entry in manualCommands"
            :key="`${entry.label}-${entry.command}`"
            class="manual-command"
          >
            <span>{{ entry.label }}</span>
            <div class="command-line">
              <code>{{ entry.command }}</code>
              <button
                class="copy-button"
                type="button"
                title="复制命令"
                @click="copyText(entry.command)"
              >
                <el-icon><CopyDocument /></el-icon>
              </button>
            </div>
          </div>
        </div>
      </section>

      <section v-if="actionOutput" class="action-console">
        <div class="console-head">
          <span>
            <el-icon v-if="actionSuccess === true" class="success-icon">
              <CircleCheck />
            </el-icon>
            <el-icon v-else class="failed-icon"><WarningFilled /></el-icon>
            上次操作日志
          </span>
        </div>
        <pre class="console-output">{{ actionOutput }}</pre>
      </section>
    </template>

    <el-dialog
      v-model="cargoHomeDialog"
      title="更改 CARGO_HOME"
      width="580px"
      :close-on-click-modal="!cargoMigrating"
      :close-on-press-escape="!cargoMigrating"
      :show-close="!cargoMigrating"
    >
      <div class="migration-dialog">
        <p class="migration-intro">
          将当前 Cargo 数据迁移到新目录，并为用户设置 <code>CARGO_HOME</code>。
          迁移期间请不要运行 cargo / rustup。
        </p>
        <div class="migration-current">
          <span>当前位置</span>
          <code>{{ cargoHome?.current }}</code>
        </div>
        <div class="migration-target">
          <el-input
            v-model="cargoHomeTarget"
            placeholder="选择新的空目录（建议非系统盘）"
            :disabled="cargoMigrating"
          />
          <el-button :disabled="cargoMigrating" @click="pickCargoHomeTarget">
            选择目录
          </el-button>
        </div>

        <div class="migration-env">
          <span class="migration-env-title">迁移后环境变量</span>
          <div><code>CARGO_HOME</code><span>{{ cargoHomeTarget || "新目录" }}</span></div>
          <div><code>PATH</code><span>追加 新目录/bin</span></div>
          <div>
            <code>RUSTUP_HOME</code>
            <span>{{ cargoHome?.rustupHome }}（保持不变）</span>
          </div>
        </div>

        <div v-if="cargoMigrationCommand" class="migration-command">
          <div class="migration-command-head">
            <span>可复制的等价命令</span>
            <button
              class="copy-button"
              type="button"
              title="复制迁移命令"
              @click="copyText(cargoMigrationCommand)"
            >
              <el-icon><CopyDocument /></el-icon>
            </button>
          </div>
          <pre>{{ cargoMigrationCommand }}</pre>
        </div>

        <el-checkbox v-model="createCargoLink" :disabled="cargoMigrating">
          在旧路径 <code>~/.cargo</code> 创建链接（推荐）
        </el-checkbox>
        <p class="migration-hint">
          创建链接后，依赖默认 <code>~/.cargo</code> 的其他应用仍可正常使用；
          不创建时，这些应用需要重新登录或手动设置 CARGO_HOME 与 PATH。
        </p>
        <div v-if="migrationError" class="migration-error">{{ migrationError }}</div>
      </div>
      <template #footer>
        <el-button :disabled="cargoMigrating" @click="cargoHomeDialog = false">
          取消
        </el-button>
        <el-button
          type="primary"
          :loading="cargoMigrating"
          :disabled="!cargoHomeTarget || Boolean(busy)"
          @click="runCargoHomeMigration"
        >
          开始迁移
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="rustupHomeDialog"
      title="更改 RUSTUP_HOME"
      width="580px"
      :close-on-click-modal="!rustupMigrating"
      :close-on-press-escape="!rustupMigrating"
      :show-close="!rustupMigrating"
    >
      <div class="migration-dialog">
        <p class="migration-intro">
          将 Rustup 工具链和下载缓存迁移到新目录，并为用户设置
          <code>RUSTUP_HOME</code>。迁移期间请不要运行 cargo / rustup / rustc。
        </p>
        <div class="migration-current">
          <span>当前位置</span>
          <code>{{ cargoHome?.rustup.current }}</code>
        </div>
        <div class="migration-target">
          <el-input
            v-model="rustupHomeTarget"
            placeholder="选择新的空目录（建议非系统盘）"
            :disabled="rustupMigrating"
          />
          <el-button :disabled="rustupMigrating" @click="pickRustupHomeTarget">
            选择目录
          </el-button>
        </div>

        <div class="migration-env">
          <span class="migration-env-title">迁移后环境变量</span>
          <div><code>RUSTUP_HOME</code><span>{{ rustupHomeTarget || "新目录" }}</span></div>
          <div>
            <code>CARGO_HOME</code>
            <span>{{ cargoHome?.current }}（保持不变）</span>
          </div>
          <div><code>PATH</code><span>保持不变</span></div>
        </div>

        <div v-if="rustupMigrationCommand" class="migration-command">
          <div class="migration-command-head">
            <span>可复制的等价命令</span>
            <button
              class="copy-button"
              type="button"
              title="复制迁移命令"
              @click="copyText(rustupMigrationCommand)"
            >
              <el-icon><CopyDocument /></el-icon>
            </button>
          </div>
          <pre>{{ rustupMigrationCommand }}</pre>
        </div>

        <el-checkbox v-model="createRustupLink" :disabled="rustupMigrating">
          在旧路径 <code>~/.rustup</code> 创建链接（推荐）
        </el-checkbox>
        <p class="migration-hint">
          创建链接后，依赖默认 <code>~/.rustup</code> 的其他应用仍可正常使用；
          不创建时，这些应用需要重新登录或手动设置 RUSTUP_HOME。
        </p>
        <div v-if="rustupMigrationError" class="migration-error">
          {{ rustupMigrationError }}
        </div>
      </div>
      <template #footer>
        <el-button :disabled="rustupMigrating" @click="rustupHomeDialog = false">
          取消
        </el-button>
        <el-button
          type="primary"
          :loading="rustupMigrating"
          :disabled="!rustupHomeTarget || Boolean(busy)"
          @click="runRustupHomeMigration"
        >
          开始迁移
        </el-button>
      </template>
    </el-dialog>
  </section>
</template>

<style scoped>
.toolchain-page {
  padding: 25px 2px 10px;
  animation: page-in 420ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.toolchain-hero {
  display: flex;
  gap: 20px;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 22px;
}

.toolchain-hero > div {
  max-width: 720px;
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

.toolchain-hero h1 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: clamp(30px, 3.1vw, 38px);
  font-weight: 650;
  line-height: 1.2;
  color: #101828;
  letter-spacing: -1.4px;
}

.toolchain-hero h1 span {
  color: #0f766e;
}

.toolchain-hero p {
  max-width: 560px;
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.8;
  color: #667085;
}

.refresh-button {
  flex: 0 0 auto;
  height: 32px;
  padding: 0 11px;
  font-size: 10.5px;
  border-radius: 8px;
}

.toolchain-error {
  display: flex;
  gap: 14px;
  align-items: flex-start;
  padding: 18px;
  background: #fff;
  border: 1px solid #fecdca;
  border-radius: 15px;
}

.error-icon {
  display: grid;
  flex: 0 0 36px;
  width: 36px;
  height: 36px;
  font-size: 18px;
  color: #d92d20;
  background: #fef3f2;
  border-radius: 10px;
  place-items: center;
}

.toolchain-error h3 {
  margin: 2px 0 4px;
  font-size: 13px;
  color: #101828;
}

.toolchain-error p {
  margin: 0;
  font-size: 10.5px;
  line-height: 1.6;
  color: #667085;
}

.retry-button {
  margin-left: auto;
}

.toolchain-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 390px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 17px;
}

.loading-mark {
  display: grid;
  width: 54px;
  height: 54px;
  margin-bottom: 16px;
  font-size: 24px;
  color: #0f766e;
  background: #edf9f7;
  border: 1px solid #d1f5ec;
  border-radius: 15px;
  place-items: center;
}

.toolchain-loading h3 {
  margin: 0;
  font-size: 13px;
  color: #344054;
}

.toolchain-loading p {
  margin: 6px 0 0;
  font-size: 10.5px;
  color: #98a2b3;
}

.operation-progress {
  position: fixed;
  right: 26px;
  bottom: 22px;
  z-index: 30;
  width: min(430px, calc(100vw - 52px));
  padding: 11px 14px;
  background: rgba(255, 255, 255, 0.97);
  border: 1px solid #d1f5ec;
  border-left: 3px solid #0f766e;
  border-radius: 10px;
  box-shadow: 0 18px 45px rgba(16, 24, 40, 0.16);
  backdrop-filter: blur(10px);
}

.operation-progress-head {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: space-between;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 10px;
  color: #0f766e;
}

.operation-progress-head > span {
  display: flex;
  gap: 6px;
  align-items: center;
}

.operation-progress-head > span:last-child {
  font-variant-numeric: tabular-nums;
  color: #98a2b3;
}

.operation-progress :deep(.el-progress) {
  margin: 8px 0 5px;
}

.operation-progress p {
  margin: 0;
  font-size: 8.5px;
  color: #98a2b3;
}

.setup-banner {
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 14px 16px;
  margin-bottom: 12px;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 12px;
}

.setup-icon {
  display: grid;
  flex: 0 0 31px;
  width: 31px;
  height: 31px;
  font-size: 15px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 8px;
  place-items: center;
}

.setup-copy h2 {
  margin: 0;
  font-size: 12px;
  color: #115e59;
}

.setup-copy p {
  margin: 3px 0 0;
  font-size: 10px;
  line-height: 1.55;
  color: #5f857e;
}

.setup-copy code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
}

.setup-versions {
  display: flex;
  gap: 6px;
  margin-left: auto;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  color: #0f766e;
}

.setup-versions span {
  padding: 4px 6px;
  background: #e4faf5;
  border-radius: 4px;
}

.rustup-banner {
  display: flex;
  flex-wrap: wrap;
  gap: 12px 18px;
  align-items: center;
  padding: 14px 16px;
  margin-bottom: 12px;
  background: #fffaf2;
  border: 1px solid #fed7aa;
  border-radius: 12px;
}

.rustup-copy {
  min-width: 210px;
  flex: 1;
}

.rustup-copy h2 {
  margin: 0;
  font-size: 12px;
  font-weight: 650;
  color: #9a3412;
}

.rustup-copy p {
  margin: 3px 0 0;
  font-size: 9.5px;
  line-height: 1.55;
  color: #9a6a4a;
}

.rustup-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  align-items: center;
}

.rustup-path-input {
  width: 250px;
}

.rustup-actions :deep(.el-input__wrapper) {
  min-height: 30px;
  box-shadow: 0 0 0 1px #f2c7a3 inset;
}

.rustup-actions :deep(.el-input__inner) {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
}

.rustup-actions :deep(.el-button) {
  height: 30px;
  padding: 0 10px;
  font-size: 9.5px;
  border-radius: 7px;
}

.current-panel {
  position: relative;
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 20px 22px;
  overflow: hidden;
  color: #f0fffb;
  background:
    radial-gradient(circle at 85% 0%, rgba(94, 234, 212, 0.17), transparent 32%),
    linear-gradient(135deg, #0f172a, #111d2f 58%, #0b3b38);
  border: 1px solid rgba(15, 118, 110, 0.22);
  border-radius: 17px;
  box-shadow: 0 18px 42px rgba(16, 24, 40, 0.12);
}

.current-panel::after {
  position: absolute;
  top: -100px;
  right: 220px;
  width: 220px;
  height: 220px;
  content: "";
  border: 1px solid rgba(148, 233, 217, 0.09);
  border-radius: 50%;
  pointer-events: none;
}

.current-mark {
  display: grid;
  flex: 0 0 46px;
  width: 46px;
  height: 46px;
  font-size: 22px;
  color: #062c29;
  background: #5eead4;
  border-radius: 13px;
  box-shadow: 0 9px 24px rgba(45, 212, 191, 0.25);
  place-items: center;
}

.current-copy {
  flex: 1;
  min-width: 0;
}

.panel-label {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 650;
  letter-spacing: 1.5px;
  color: #8fb6b0;
  text-transform: uppercase;
}

.current-copy h2 {
  margin: 4px 0 4px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: clamp(32px, 3.2vw, 42px);
  font-weight: 620;
  line-height: 1;
  letter-spacing: -1.6px;
}

.current-copy code {
  display: block;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  color: #94c9c0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.current-build {
  display: block;
  margin-top: 3px;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #77a39b;
}

.current-latest small {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  color: #8fb6b0;
  font-variant-numeric: tabular-nums;
}

.current-latest {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: 3px;
  align-items: flex-end;
  padding-left: 20px;
  border-left: 1px solid rgba(255, 255, 255, 0.09);
}

.current-latest span {
  font-size: 9px;
  color: #789489;
}

.current-latest strong {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 18px;
  font-weight: 600;
  color: #d4f8f0;
}

.current-latest em {
  padding: 3px 6px;
  font-size: 8px;
  font-style: normal;
  color: #9af5e5;
  background: rgba(45, 212, 191, 0.12);
  border-radius: 4px;
}

.current-latest em.update-hint {
  color: #fde68a;
  background: rgba(251, 191, 36, 0.12);
}

.latest-warning {
  display: flex;
  gap: 7px;
  align-items: center;
  padding: 9px 11px;
  margin-top: 9px;
  font-size: 9px;
  color: #93370d;
  background: #fffaeb;
  border: 1px solid #fedf89;
  border-radius: 8px;
}

.latest-warning button {
  padding: 0;
  margin-left: auto;
  font-size: 9px;
  color: #b54708;
  text-decoration: underline;
  cursor: pointer;
  background: transparent;
  border: 0;
}

.cargo-home-card {
  padding: 15px 16px;
  margin-top: 12px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 12px;
}

.env-head {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  justify-content: space-between;
}

.env-head h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 12px;
  font-weight: 650;
  color: #344054;
}

.env-sub {
  margin: 4px 0 0;
  font-size: 8.5px;
  color: #98a2b3;
}

.link-badges {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  justify-content: flex-end;
}

.link-badge {
  padding: 3px 6px;
  font-size: 8px;
  color: #667085;
  background: #f2f4f7;
  border-radius: 4px;
}

.link-badge.ok {
  color: #0f766e;
  background: #edf9f7;
}

.link-badge.warn {
  color: #b54708;
  background: #fffaeb;
}

.env-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 7px;
  margin-top: 11px;
}

.env-item {
  min-width: 0;
  padding: 8px 9px;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 7px;
}

.env-item-head {
  display: flex;
  gap: 6px;
  align-items: center;
  justify-content: space-between;
}

.env-item-head code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  font-weight: 600;
  color: #0f766e;
}

.env-item-head span {
  font-size: 7.5px;
  color: #98a2b3;
  white-space: nowrap;
}

.env-item p {
  margin: 4px 0 0;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #667085;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.env-configure {
  padding: 0;
  margin-top: 5px;
  font-size: 8px;
  color: #0f766e;
  text-decoration: underline;
  cursor: pointer;
  background: transparent;
  border: 0;
}

.env-configure:hover {
  color: #115e59;
}

.link-detail {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 6px 8px;
  margin-top: 7px;
  overflow: hidden;
  font-size: 8.5px;
  color: #667085;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 6px;
}

.link-detail code {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #0f766e;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cargo-home-footer {
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: space-between;
  padding-top: 10px;
  margin-top: 10px;
  border-top: 1px solid #f2f4f7;
}

.cargo-home-footer p {
  max-width: 620px;
  margin: 0;
  font-size: 8.5px;
  line-height: 1.6;
  color: #98a2b3;
}

.storage-actions {
  display: flex;
  flex: 0 0 auto;
  flex-wrap: wrap;
  gap: 6px;
  justify-content: flex-end;
}

.cargo-home-footer :deep(.el-button) {
  flex: 0 0 auto;
  height: 29px;
  padding: 0 10px;
  font-size: 9px;
  border-radius: 7px;
}

.cargo-home-error {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 8px 10px;
  margin-top: 9px;
  font-size: 9px;
  color: #b54708;
  background: #fffaeb;
  border: 1px solid #fedf89;
  border-radius: 7px;
}

.migration-dialog {
  font-size: 10px;
  color: #475467;
}

.migration-intro {
  margin: 0 0 12px;
  line-height: 1.7;
}

.migration-intro code,
.migration-hint code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  color: #0f766e;
}

.migration-current {
  display: flex;
  gap: 8px;
  align-items: center;
  padding: 8px 9px;
  margin-bottom: 8px;
  background: #f8fafc;
  border-radius: 6px;
}

.migration-current span {
  font-size: 9px;
  color: #98a2b3;
}

.migration-current code {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  color: #475467;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.migration-target {
  display: flex;
  gap: 7px;
}

.migration-target :deep(.el-input__wrapper) {
  min-height: 31px;
}

.migration-target :deep(.el-button) {
  height: 31px;
  padding: 0 10px;
  font-size: 9px;
  border-radius: 7px;
}

.migration-env {
  padding: 9px 10px;
  margin: 10px 0;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 7px;
}

.migration-env-title {
  display: block;
  margin-bottom: 5px;
  font-size: 8px;
  font-weight: 600;
  color: #0f766e;
  letter-spacing: 0.5px;
}

.migration-env > div {
  display: flex;
  gap: 8px;
  padding: 2px 0;
  font-size: 9px;
  color: #667085;
}

.migration-env code {
  flex: 0 0 84px;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #0f766e;
}

.migration-hint {
  margin: 6px 0 0;
  font-size: 9px;
  line-height: 1.65;
  color: #98a2b3;
}

.migration-error {
  padding: 7px 8px;
  margin-top: 9px;
  font-size: 9px;
  color: #b42318;
  background: #fef3f2;
  border: 1px solid #fecdca;
  border-radius: 6px;
}

.rustup-source-card {
  padding: 15px 16px;
  margin-top: 12px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 12px;
}

.source-head {
  display: flex;
  gap: 12px;
  align-items: flex-start;
  justify-content: space-between;
}

.source-head h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 12px;
  font-weight: 650;
  color: #344054;
}

.source-head p {
  margin: 4px 0 0;
  font-size: 8.5px;
  color: #98a2b3;
}

.source-current-badge {
  flex: 0 0 auto;
  padding: 4px 8px;
  font-size: 8px;
  font-weight: 600;
  color: #0f766e;
  background: #ecfdf5;
  border: 1px solid #c8f1e7;
  border-radius: 999px;
}

.source-current {
  display: grid;
  grid-template-columns: 90px minmax(0, 1fr) auto;
  gap: 6px;
  align-items: center;
  padding: 7px 8px;
  margin-top: 9px;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 7px;
}

.source-current > span {
  grid-column: 1;
  grid-row: 1;
  font-size: 8px;
  color: #98a2b3;
}

.source-current code {
  grid-column: 2;
  min-width: 0;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #475467;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-current > code:first-of-type {
  grid-row: 1;
}

.source-current > code:last-of-type {
  grid-row: 2;
}

.source-current small {
  grid-column: 3;
  grid-row: 1 / span 2;
  font-size: 7.5px;
  color: #98a2b3;
  white-space: nowrap;
}

.source-options {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  margin-top: 8px;
}

.source-option {
  height: 25px;
  padding: 0 9px;
  font-size: 8px;
  color: #667085;
  cursor: pointer;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 6px;
}

.source-option:hover {
  color: #0f766e;
  border-color: #b8e8df;
}

.source-option.active {
  color: #0f766e;
  background: #f0fdfa;
  border-color: #99f6e4;
  font-weight: 650;
}

.source-custom {
  display: grid;
  gap: 6px;
  margin-top: 8px;
}

.source-custom :deep(.el-input__wrapper) {
  min-height: 28px;
  padding: 0 9px;
  border-radius: 6px;
  box-shadow: 0 0 0 1px #e4e7ec inset;
}

.source-custom :deep(.el-input__inner) {
  height: 28px;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
}

.source-preview {
  display: grid;
  gap: 4px;
  margin-top: 8px;
}

.source-preview > div {
  display: grid;
  grid-template-columns: 130px minmax(0, 1fr);
  gap: 7px;
  align-items: center;
}

.source-preview span {
  font-size: 7.5px;
  color: #98a2b3;
}

.source-preview code {
  min-width: 0;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #475467;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-command {
  padding: 7px 8px;
  margin-top: 8px;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 7px;
}

.source-command-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
  font-size: 8px;
  font-weight: 600;
  color: #667085;
}

.source-command pre {
  max-height: 100px;
  padding: 0;
  margin: 0;
  overflow: auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  line-height: 1.6;
  color: #475467;
  white-space: pre-wrap;
  word-break: break-all;
}

.source-actions {
  display: flex;
  gap: 7px;
  justify-content: flex-end;
  margin-top: 9px;
}

.source-actions :deep(.el-button) {
  height: 27px;
  padding: 0 11px;
  font-size: 8px;
  border-radius: 6px;
}

.source-hint {
  margin: 6px 0 0;
  font-size: 7.5px;
  color: #98a2b3;
  text-align: right;
}

.source-backup-note {
  margin: 3px 0 0;
  overflow: hidden;
  font-size: 7.5px;
  color: #98a2b3;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-backup-note code {
  font-family: "Cascadia Code", "Consolas", monospace;
  color: #667085;
}

.channel-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-top: 12px;
}

.channel-card {
  display: flex;
  flex-direction: column;
  gap: 11px;
  padding: 16px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 13px;
  box-shadow: 0 9px 26px rgba(16, 24, 40, 0.04);
}

.channel-card.default {
  background:
    radial-gradient(circle at 100% 0%, rgba(45, 212, 191, 0.07), transparent 38%),
    #fff;
  border-color: #0f766e;
}

.channel-head {
  display: flex;
  gap: 10px;
  align-items: flex-start;
  justify-content: space-between;
}

.channel-name {
  display: block;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 14px;
  font-weight: 650;
  color: #101828;
  letter-spacing: -0.2px;
}

.channel-sub {
  display: block;
  margin-top: 2px;
  font-size: 8.5px;
  color: #98a2b3;
}

.status-badge {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 3px 6px;
  font-size: 8px;
  font-weight: 600;
  color: #475467;
  background: #f2f4f7;
  border-radius: 4px;
}

.status-badge.active {
  color: #0f766e;
  background: #edf9f7;
}

.status-badge.muted {
  color: #98a2b3;
  background: #f8fafc;
}

.status-badge .el-icon {
  font-size: 9px;
}

.version-pair {
  display: flex;
  gap: 14px;
  align-items: flex-end;
  padding: 10px 12px;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 8px;
}

.version-pair > div {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.version-label {
  font-size: 8px;
  color: #98a2b3;
}

.version-pair strong {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 16px;
  font-weight: 620;
  color: #101828;
  font-variant-numeric: tabular-nums;
}

.version-pair > div:last-child strong {
  color: #0f766e;
}

.build-label {
  margin-top: 1px;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 7.5px;
  color: #98a2b3;
  font-variant-numeric: tabular-nums;
}

.version-arrow {
  padding-bottom: 2px;
  font-size: 11px;
  color: #b0b8c4;
}

.channel-state {
  flex: 1;
  margin: 0;
  font-size: 9px;
  line-height: 1.55;
  color: #667085;
}

.channel-state.ok {
  color: #0f766e;
}

.channel-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  align-items: center;
}

.card-commands {
  display: flex;
  flex-direction: column;
  gap: 4px;
  margin-top: 1px;
}

.command-line {
  display: flex;
  gap: 6px;
  align-items: flex-start;
  min-width: 0;
  padding: 5px 7px;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 6px;
}

.command-line code {
  flex: 1;
  min-width: 0;
  padding-top: 1px;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  line-height: 1.55;
  color: #475467;
  word-break: break-all;
}

.command-label {
  flex: 0 0 auto;
  padding-top: 1px;
  font-size: 7.5px;
  color: #98a2b3;
  white-space: nowrap;
}

.copy-button {
  display: grid;
  flex: 0 0 20px;
  width: 20px;
  height: 20px;
  padding: 0;
  font-size: 10px;
  color: #667085;
  cursor: pointer;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 5px;
  place-items: center;
}

.copy-button:hover {
  color: #0f766e;
  border-color: #b8e8df;
}

.manual-commands {
  padding: 15px 16px;
  margin-top: 14px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 12px;
}

.manual-commands-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  margin-bottom: 9px;
}

.manual-commands-head h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 12px;
  font-weight: 650;
  color: #344054;
}

.manual-commands-head > span {
  font-size: 8px;
  color: #98a2b3;
}

.manual-command-list {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

.manual-command {
  display: grid;
  grid-template-columns: 190px minmax(0, 1fr);
  gap: 8px;
  align-items: start;
}

.manual-command > span {
  padding-top: 5px;
  overflow: hidden;
  font-size: 8px;
  color: #667085;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.migration-command {
  padding: 8px 9px;
  margin: 8px 0;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 7px;
}

.migration-command-head {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 5px;
  font-size: 8px;
  font-weight: 600;
  color: #667085;
}

.migration-command pre {
  max-height: 150px;
  padding: 0;
  margin: 0;
  overflow: auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  line-height: 1.6;
  color: #475467;
  white-space: pre-wrap;
  word-break: break-all;
}

.channel-actions :deep(.el-button) {
  height: 28px;
  padding: 0 10px;
  font-size: 9.5px;
  border-radius: 7px;
}

.switch-button {
  color: #475467;
}

.history-card {
  padding: 15px 16px;
  margin-top: 12px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 12px;
}

.history-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  margin-bottom: 10px;
}

.section-kicker {
  display: block;
  margin-bottom: 3px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 650;
  letter-spacing: 1.7px;
  color: #8fb8b1;
}

.history-head h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 12px;
  font-weight: 650;
  color: #344054;
}

.history-warning {
  font-size: 8.5px;
  color: #b54708;
}

.history-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  align-items: center;
}

.history-select {
  width: 132px;
}

.history-actions :deep(.el-select__wrapper) {
  min-height: 30px;
  box-shadow: 0 0 0 1px #e4e7ec inset;
}

.history-divider {
  width: 1px;
  height: 20px;
  margin: 0 3px;
  background: #e4e7ec;
}

.custom-version {
  width: 132px;
}

.history-actions :deep(.el-input__wrapper) {
  min-height: 30px;
  box-shadow: 0 0 0 1px #e4e7ec inset;
}

.history-actions :deep(.el-input__inner) {
  font-size: 9.5px;
}

.history-actions :deep(.el-button) {
  height: 30px;
  padding: 0 10px;
  font-size: 9.5px;
  border-radius: 7px;
}

.installed-section {
  padding: 20px 3px 0;
}

.section-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 8px;
}

.section-heading h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  font-weight: 650;
  color: #344054;
}

.section-heading > span {
  font-size: 9px;
  color: #98a2b3;
}

.toolchain-list {
  overflow: hidden;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 11px;
}

.toolchain-row {
  display: flex;
  gap: 9px;
  align-items: center;
  padding: 10px 13px;
  border-bottom: 1px solid #f2f4f7;
}

.toolchain-row:last-child {
  border-bottom: 0;
}

.toolchain-row.default {
  background: #f7fffd;
}

.toolchain-dot {
  width: 6px;
  height: 6px;
  background: #d0d5dd;
  border-radius: 50%;
}

.toolchain-row.default .toolchain-dot {
  background: #14b8a6;
  box-shadow: 0 0 0 3px rgba(20, 184, 166, 0.12);
}

.toolchain-name {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9.5px;
  color: #475467;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.toolchain-version {
  margin-left: auto;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9.5px;
  color: #98a2b3;
}

.active-label {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 3px 6px;
  font-size: 8.5px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 4px;
}

.active-label .el-icon {
  font-size: 9px;
}

.row-switch {
  height: 24px;
  padding: 0 5px;
  font-size: 8.5px;
  color: #667085;
}

.row-delete {
  width: 26px;
  height: 24px;
  padding: 0;
  font-size: 13px;
  color: #98a2b3;
}

.row-delete:hover {
  color: #d92d20;
}

.action-console {
  padding: 12px 14px;
  margin-top: 12px;
  background: #0b1422;
  border: 1px solid #1e2b3c;
  border-radius: 10px;
}

.console-head {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: space-between;
  font-size: 9px;
  color: #a9b8c9;
}

.console-head > span {
  display: flex;
  gap: 5px;
  align-items: center;
}

.console-head .el-icon {
  font-size: 11px;
}

.console-hint {
  color: #64748b;
}

.console-output {
  max-height: 180px;
  padding: 0;
  margin: 9px 0 0;
  overflow: auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  line-height: 1.7;
  color: #a9d8d2;
  white-space: pre-wrap;
}

.success-icon {
  color: #2dd4bf;
}

.failed-icon {
  color: #f97066;
}

.spinning {
  animation: spin 1.4s linear infinite;
}

@keyframes page-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 900px) {
  .toolchain-hero {
    align-items: flex-start;
  }

  .channel-grid,
  .env-grid {
    grid-template-columns: 1fr;
  }

  .cargo-home-footer {
    flex-direction: column;
    align-items: flex-start;
  }

  .manual-command {
    grid-template-columns: 1fr;
    gap: 2px;
  }

  .source-current {
    grid-template-columns: 1fr;
  }

  .source-current > span,
  .source-current code,
  .source-current small {
    grid-column: auto;
    grid-row: auto;
  }

  .source-preview > div {
    grid-template-columns: 1fr;
    gap: 2px;
  }

  .current-panel {
    align-items: flex-start;
  }

  .current-latest {
    display: none;
  }

  .history-divider {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .toolchain-page,
  .spinning {
    animation: none;
  }
}
</style>

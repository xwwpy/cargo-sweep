<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ElMessage } from "element-plus";
import {
  CircleCheck,
  Clock,
  Cpu,
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
  installRustup,
  installRustToolchain,
  setDefaultRustToolchain,
  setRustupPath,
  updateRustToolchain,
} from "../api";
import type {
  LatestRustToolchain,
  LatestRustVersions,
  RustChannel,
  RustToolchainStatus,
} from "../types";

interface ChannelView {
  key: "stable" | "nightly";
  label: string;
  installed: boolean;
  toolchain: string | null;
  currentRelease: string | null;
  currentFull: string | null;
  latest: LatestRustToolchain | null;
  latestRelease: string | null;
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
const busy = ref<string | null>(null);
const actionOutput = ref<string | null>(null);
const actionSuccess = ref<boolean | null>(null);
const elapsed = ref(0);
const historyTarget = ref("");
const customTarget = ref("");
const rustupPathInput = ref("");
let timer: number | undefined;

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
const currentVersion = computed(() => status.value?.version ?? null);
const latestVersion = computed(() => activeLatest.value?.release ?? null);
const currentIsLatest = computed(() => {
  const current = currentChannel.value;
  if (!current?.installed || !current.latest) {
    return null;
  }
  return sameVersion(current.currentFull, `rustc ${current.latest.version}`);
});
const historyOptions = computed(() => latest.value?.historicalVersions ?? []);
const installedToolchains = computed(() => status.value?.toolchains ?? []);
const busyLabel = computed(() => busy.value ?? "");
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
    currentRelease: info?.release ?? null,
    currentFull,
    latest: latestInfo,
    latestRelease: latestInfo?.release ?? null,
    updateAvailable:
      info?.installed && latestInfo && currentFull
        ? !sameVersion(currentFull, `rustc ${latestInfo.version}`)
        : null,
    active: Boolean(info?.active),
    default: Boolean(info?.default),
  };
}

function sameVersion(left: string | null, right: string | null): boolean {
  if (!left || !right) {
    return false;
  }
  return left.replace(/^rustc\s+/, "").trim() === right.replace(/^rustc\s+/, "").trim();
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

async function load() {
  loading.value = true;
  error.value = null;
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
          <span v-if="latest?.stable">Stable {{ latest.stable.release }}</span>
          <span v-if="latest?.nightly">Nightly {{ latest.nightly.release }}</span>
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
      </div>

      <section v-if="status.installed" class="current-panel">
        <div class="current-mark"><el-icon><Cpu /></el-icon></div>
        <div class="current-copy">
          <span class="panel-label">当前默认工具链</span>
          <h2>{{ currentVersion || "未知版本" }}</h2>
          <code>{{ status.activeToolchain || status.defaultToolchain || "未设置默认工具链" }}</code>
        </div>
        <div class="current-latest">
          <span>最新版本</span>
          <strong>
            <template v-if="latestLoading">检测中…</template>
            <template v-else>{{ latestVersion || "未获取" }}</template>
          </strong>
          <em v-if="currentIsLatest === true">已是最新</em>
          <em v-else-if="currentIsLatest === false" class="update-hint">有新版本</em>
        </div>
      </section>

      <div v-if="latestError" class="latest-warning">
        <el-icon><WarningFilled /></el-icon>
        <span>最新版本获取失败：{{ latestError }}</span>
        <button type="button" @click="retryLatest">重试</button>
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
            </div>
            <span class="version-arrow">→</span>
            <div>
              <span class="version-label">最新</span>
              <strong>
                <template v-if="latestLoading">检测中…</template>
                <template v-else>{{ channel.latestRelease || "未知" }}</template>
              </strong>
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
            <span class="toolchain-version">{{ toolchain.release || "未知" }}</span>
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
          </div>
        </div>
      </section>

      <section v-if="busy || actionOutput" class="action-console">
        <div class="console-head">
          <span>
            <el-icon v-if="busy" class="spinning"><Clock /></el-icon>
            <el-icon v-else-if="actionSuccess === true" class="success-icon">
              <CircleCheck />
            </el-icon>
            <el-icon v-else class="failed-icon"><WarningFilled /></el-icon>
            {{ busy ? `${busyLabel} · ${elapsedLabel}` : "上次操作日志" }}
          </span>
          <span v-if="busy" class="console-hint">请稍候，下载可能需要几分钟</span>
        </div>
        <el-progress
          v-if="busy"
          :percentage="100"
          :show-text="false"
          :indeterminate="true"
          :duration="1.8"
          :stroke-width="3"
        />
        <pre v-if="actionOutput" class="console-output">{{ actionOutput }}</pre>
      </section>
    </template>
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

  .channel-grid {
    grid-template-columns: 1fr;
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

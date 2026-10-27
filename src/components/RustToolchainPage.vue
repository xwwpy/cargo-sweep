<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ElMessage } from "element-plus";
import {
  CircleCheck,
  Cpu,
  Download,
  InfoFilled,
  Refresh,
  WarningFilled,
} from "@element-plus/icons-vue";
import { detectRustToolchain, errorMessage, updateRustToolchain } from "../api";
import type { RustToolchainStatus } from "../types";

const status = ref<RustToolchainStatus | null>(null);
const loading = ref(true);
const updating = ref(false);
const error = ref<string | null>(null);
const output = ref<string | null>(null);
const updateSuccess = ref<boolean | null>(null);
const elapsed = ref(0);
let timer: number | undefined;

const channelLabel = computed(() => {
  const channel = status.value?.channel;
  if (!channel) {
    return "未知通道";
  }
  if (channel === "pinned") {
    return "固定版本";
  }
  if (channel === "custom") {
    return "自定义";
  }
  return channel;
});

const updateCommand = computed(() => {
  const toolchain = status.value?.activeToolchain;
  return toolchain
    ? `rustup update --no-self-update ${toolchain}`
    : "rustup update --no-self-update";
});

const canUpdate = computed(
  () => status.value !== null && status.value.rustupVersion !== null && !updating.value,
);

const elapsedLabel = computed(() => {
  const minutes = Math.floor(elapsed.value / 60);
  const seconds = elapsed.value % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
});

async function load() {
  loading.value = true;
  error.value = null;
  try {
    status.value = await detectRustToolchain();
  } catch (reason) {
    error.value = errorMessage(reason);
  } finally {
    loading.value = false;
  }
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

async function update() {
  if (updating.value || !status.value) {
    return;
  }

  updating.value = true;
  output.value = null;
  updateSuccess.value = null;
  startTimer();

  try {
    const result = await updateRustToolchain();
    status.value = result.status;
    output.value = result.output;
    updateSuccess.value = result.success;
    if (result.success) {
      ElMessage.success(`Rust 工具链已更新到 ${result.status.version}`);
    } else {
      ElMessage.error("更新未完成，请查看下方日志");
    }
  } catch (reason) {
    updateSuccess.value = false;
    output.value = errorMessage(reason);
    ElMessage.error("更新失败，请查看下方日志");
  } finally {
    stopTimer();
    updating.value = false;
  }
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
        <h1>工具链，<span>保持在最新。</span></h1>
        <p>查看当前 Rust 版本，一键更新活动工具链。</p>
      </div>
      <el-button class="refresh-button" :loading="loading" @click="load">
        <el-icon v-if="!loading"><Refresh /></el-icon>
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
      <section class="version-panel">
        <div class="version-mark"><el-icon><Cpu /></el-icon></div>
        <div class="version-copy">
          <span class="version-label">当前 Rust 版本</span>
          <div class="version-line">
            <h2>{{ status.version }}</h2>
            <span class="channel-pill">{{ channelLabel }}</span>
          </div>
          <code :title="status.rustcVersion">{{ status.rustcVersion }}</code>
        </div>
        <div class="version-meta">
          <div>
            <span>活动工具链</span>
            <strong>{{ status.activeToolchain || "未检测到" }}</strong>
          </div>
          <div>
            <span>目标平台</span>
            <strong>{{ status.host || "未知" }}</strong>
          </div>
        </div>
      </section>

      <section class="version-facts">
        <article>
          <span>Cargo</span>
          <strong>{{ status.cargoVersion || "未检测到" }}</strong>
        </article>
        <article>
          <span>rustup</span>
          <strong>{{ status.rustupVersion || "未检测到" }}</strong>
        </article>
        <article>
          <span>已安装工具链</span>
          <strong>{{ status.toolchains.length }}</strong>
        </article>
      </section>

      <section class="update-card" :class="{ failed: updateSuccess === false }">
        <div class="update-heading">
          <div class="update-icon"><el-icon><Download /></el-icon></div>
          <div>
            <h3>更新当前工具链</h3>
            <p>通过 rustup 获取当前通道的最新版本，不改动其他工具链。</p>
          </div>
          <span v-if="updating" class="updating-pill">
            <el-icon class="spinning"><Refresh /></el-icon>
            {{ elapsedLabel }}
          </span>
          <el-icon v-else-if="updateSuccess === true" class="success-icon">
            <CircleCheck />
          </el-icon>
          <el-icon v-else-if="updateSuccess === false" class="failed-icon">
            <WarningFilled />
          </el-icon>
        </div>

        <div class="command-line">
          <span>$</span>
          <code>{{ updateCommand }}</code>
        </div>

        <div v-if="updating" class="update-progress">
          <el-progress
            :percentage="100"
            :show-text="false"
            :indeterminate="true"
            :duration="1.8"
            :stroke-width="3"
          />
          <span>正在下载并安装，可能需要几分钟</span>
        </div>

        <div class="update-actions">
          <span class="update-hint">
            <el-icon><InfoFilled /></el-icon>
            {{ status.rustupVersion ? "使用 rustup 更新" : "未检测到 rustup" }}
          </span>
          <el-button
            type="primary"
            class="update-button"
            :disabled="!canUpdate"
            :loading="updating"
            @click="update"
          >
            <el-icon v-if="!updating"><Download /></el-icon>
            {{ updating ? "更新中" : "一键更新" }}
          </el-button>
        </div>

        <pre
          v-if="output"
          class="update-log"
          :class="{ failed: updateSuccess === false }"
        >{{ output }}</pre>
      </section>

      <section class="toolchain-section">
        <div class="section-heading">
          <div>
            <span class="section-kicker">INSTALLED</span>
            <h2>已安装工具链</h2>
          </div>
        </div>

        <div v-if="status.toolchains.length > 0" class="toolchain-list">
          <div
            v-for="toolchain in status.toolchains"
            :key="toolchain.name"
            class="toolchain-row"
            :class="{ active: toolchain.active }"
          >
            <span class="toolchain-dot" />
            <span class="toolchain-name">{{ toolchain.name }}</span>
            <span v-if="toolchain.active" class="active-label">
              <el-icon><CircleCheck /></el-icon>
              当前
            </span>
          </div>
        </div>
        <p v-else class="empty-toolchains">暂未读取到工具链列表。</p>
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

.version-panel {
  position: relative;
  display: flex;
  gap: 18px;
  align-items: center;
  padding: 23px 24px;
  overflow: hidden;
  color: #f0fffb;
  background:
    radial-gradient(circle at 85% 0%, rgba(94, 234, 212, 0.17), transparent 32%),
    linear-gradient(135deg, #0f172a, #111d2f 58%, #0b3b38);
  border: 1px solid rgba(15, 118, 110, 0.22);
  border-radius: 17px;
  box-shadow: 0 18px 42px rgba(16, 24, 40, 0.12);
}

.version-panel::after {
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

.version-mark {
  display: grid;
  flex: 0 0 52px;
  width: 52px;
  height: 52px;
  font-size: 24px;
  color: #062c29;
  background: #5eead4;
  border-radius: 14px;
  box-shadow: 0 9px 24px rgba(45, 212, 191, 0.25);
  place-items: center;
}

.version-copy {
  flex: 1;
  min-width: 0;
}

.version-label {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 650;
  letter-spacing: 1.5px;
  color: #8fb6b0;
  text-transform: uppercase;
}

.version-line {
  display: flex;
  gap: 10px;
  align-items: center;
  margin: 4px 0 5px;
}

.version-line h2 {
  margin: 0;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: clamp(34px, 3.4vw, 44px);
  font-weight: 620;
  line-height: 1;
  letter-spacing: -1.8px;
}

.channel-pill {
  padding: 4px 7px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 650;
  color: #9af5e5;
  text-transform: uppercase;
  border: 1px solid rgba(94, 234, 212, 0.2);
  border-radius: 5px;
}

.version-copy code {
  display: block;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9.5px;
  color: #94c9c0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.version-meta {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: 8px;
  max-width: 330px;
  padding-left: 20px;
  border-left: 1px solid rgba(255, 255, 255, 0.09);
}

.version-meta div {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: flex-end;
  font-size: 9px;
  color: #789489;
}

.version-meta strong {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  font-weight: 500;
  color: #c5ded9;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.version-facts {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 11px;
  margin-top: 12px;
}

.version-facts article {
  display: flex;
  flex-direction: column;
  gap: 5px;
  padding: 13px 15px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 11px;
}

.version-facts span {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 650;
  letter-spacing: 1px;
  color: #98a2b3;
  text-transform: uppercase;
}

.version-facts strong {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 10.5px;
  font-weight: 500;
  color: #344054;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.update-card {
  padding: 17px;
  margin-top: 12px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 13px;
  box-shadow: 0 9px 26px rgba(16, 24, 40, 0.04);
}

.update-card.failed {
  border-color: #fecdca;
}

.update-heading {
  display: flex;
  gap: 11px;
  align-items: flex-start;
}

.update-icon {
  display: grid;
  flex: 0 0 31px;
  width: 31px;
  height: 31px;
  font-size: 14px;
  color: #0f766e;
  background: #edf9f7;
  border: 1px solid #d1f5ec;
  border-radius: 8px;
  place-items: center;
}

.update-heading h3 {
  margin: 1px 0 3px;
  font-size: 12px;
  font-weight: 650;
  color: #101828;
}

.update-heading p {
  margin: 0;
  font-size: 9.5px;
  line-height: 1.55;
  color: #667085;
}

.updating-pill {
  display: flex;
  gap: 5px;
  align-items: center;
  padding: 4px 7px;
  margin-left: auto;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 5px;
}

.success-icon {
  margin-left: auto;
  font-size: 19px;
  color: #0f9f8f;
}

.failed-icon {
  margin-left: auto;
  font-size: 19px;
  color: #d92d20;
}

.command-line {
  display: flex;
  gap: 7px;
  align-items: center;
  padding: 8px 10px;
  margin-top: 12px;
  overflow: hidden;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 7px;
}

.command-line > span {
  flex: 0 0 auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  color: #98a2b3;
}

.command-line code {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #475467;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.update-progress {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 12px;
}

.update-progress span {
  font-size: 9px;
  color: #98a2b3;
}

.update-progress :deep(.el-progress-bar__outer) {
  background: #e9eef2;
}

.update-actions {
  display: flex;
  gap: 12px;
  align-items: center;
  justify-content: space-between;
  margin-top: 13px;
}

.update-hint {
  display: flex;
  gap: 4px;
  align-items: center;
  font-size: 8.5px;
  color: #98a2b3;
}

.update-hint .el-icon {
  font-size: 10px;
}

.update-button {
  height: 32px;
  padding: 0 13px;
  font-size: 10.5px;
  border-radius: 8px;
}

.update-log {
  max-height: 190px;
  padding: 11px 13px;
  margin: 12px 0 0;
  overflow: auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  line-height: 1.7;
  color: #c9e6e1;
  white-space: pre-wrap;
  background: #0b1422;
  border: 1px solid #1e2b3c;
  border-radius: 8px;
}

.update-log.failed {
  color: #fecaca;
  border-color: #5a2627;
}

.toolchain-section {
  padding: 21px 3px 0;
}

.section-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 9px;
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

.section-heading h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  font-weight: 650;
  color: #344054;
}

.section-hint {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #98a2b3;
}

.toolchain-list {
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 11px;
}

.toolchain-row {
  display: flex;
  gap: 9px;
  align-items: center;
  padding: 11px 13px;
  border-bottom: 1px solid #f2f4f7;
}

.toolchain-row:last-child {
  border-bottom: 0;
}

.toolchain-row.active {
  background: #f7fffd;
}

.toolchain-dot {
  width: 6px;
  height: 6px;
  background: #d0d5dd;
  border-radius: 50%;
}

.toolchain-row.active .toolchain-dot {
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

.toolchain-row.active .toolchain-name {
  color: #0f766e;
}

.active-label {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 3px 6px;
  margin-left: auto;
  font-size: 8.5px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 4px;
}

.active-label .el-icon {
  font-size: 9px;
}

.empty-toolchains {
  padding: 16px;
  margin: 0;
  font-size: 10px;
  color: #98a2b3;
  text-align: center;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 11px;
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

@media (max-width: 1120px) {
  .version-meta {
    max-width: 280px;
  }
}

@media (max-width: 900px) {
  .toolchain-hero {
    align-items: flex-start;
  }

  .version-panel {
    align-items: flex-start;
  }

  .version-meta {
    display: none;
  }

  .version-facts {
    grid-template-columns: 1fr;
  }
}

@media (prefers-reduced-motion: reduce) {
  .toolchain-page,
  .spinning {
    animation: none;
  }
}
</style>

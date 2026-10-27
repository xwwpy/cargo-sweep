<script setup lang="ts">
import { computed } from "vue";
import { Close, Download, RefreshRight } from "@element-plus/icons-vue";
import type { UpdateInfo } from "../composables/useUpdater";
import { formatBytes } from "../lib/format";
import { APP_VERSION } from "../lib/version";

const props = defineProps<{
  modelValue: boolean;
  update: UpdateInfo | null;
  installing: boolean;
  preparingRestart: boolean;
  progressPercent: number;
  downloaded: number;
  contentLength: number;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  install: [];
}>();

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit("update:modelValue", value),
});

const releaseNotes = computed(() => {
  const body = props.update?.body?.trim();
  if (!body) {
    return "本次更新包含界面优化、稳定性改进与问题修复。";
  }
  return body
    .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
    .replace(/^#{1,6}\s*/gm, "")
    .replace(/^[-*]\s+/gm, "• ")
    .replace(/\*\*/g, "");
});

const releaseDate = computed(() => {
  if (!props.update?.date) {
    return "";
  }
  const date = new Date(props.update.date);
  if (Number.isNaN(date.getTime())) {
    return "";
  }
  return new Intl.DateTimeFormat("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
  }).format(date);
});

function close() {
  if (!props.installing) {
    visible.value = false;
  }
}

function handleBeforeClose(done: () => void) {
  if (!props.installing) {
    done();
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    class="update-dialog"
    width="540px"
    align-center
    append-to-body
    :show-close="false"
    :close-on-click-modal="!installing"
    :close-on-press-escape="!installing"
    :before-close="handleBeforeClose"
  >
    <div class="update-body">
      <button
        class="dialog-close"
        type="button"
        aria-label="关闭"
        :disabled="installing"
        @click="close"
      >
        <el-icon><Close /></el-icon>
      </button>

      <div class="update-mark">
        <el-icon><Download /></el-icon>
        <span class="mark-ring" />
      </div>

      <div class="eyebrow">UPDATE AVAILABLE</div>
      <h2>新版本已准备就绪</h2>
      <p class="lead">升级到最新版本，获得更清爽的界面与更稳定的清理体验。</p>

      <div class="version-track">
        <div class="version-block">
          <span>当前版本</span>
          <strong>v{{ update?.currentVersion ?? APP_VERSION }}</strong>
        </div>
        <div class="version-arrow">→</div>
        <div class="version-block next">
          <span>最新版本</span>
          <strong>v{{ update?.version ?? "—" }}</strong>
        </div>
        <span v-if="releaseDate" class="release-date">{{ releaseDate }}</span>
      </div>

      <div class="notes-label">
        <span>更新内容</span>
        <span class="notes-hint">来自 GitHub Releases</span>
      </div>
      <div class="release-notes">{{ releaseNotes }}</div>

      <div v-if="installing" class="install-progress">
        <div class="progress-head">
          <span>{{ preparingRestart ? "正在启动安装程序…" : "正在下载更新…" }}</span>
          <span v-if="contentLength > 0" class="progress-size">
            {{ formatBytes(downloaded) }} / {{ formatBytes(contentLength) }}
          </span>
        </div>
        <el-progress
          :percentage="progressPercent"
          :show-text="false"
          :indeterminate="contentLength <= 0"
          :duration="1.4"
          :stroke-width="5"
          color="#0f766e"
        />
        <p class="progress-note">安装过程中请保持应用开启，完成后会自动重启。</p>
      </div>

      <div v-else class="update-note">
        <el-icon><RefreshRight /></el-icon>
        <span>更新会覆盖安装当前版本，已保存的扫描目录设置保持不变。</span>
      </div>
    </div>

    <template #footer>
      <div class="update-footer">
        <el-button class="later-button" :disabled="installing" @click="close">稍后再说</el-button>
        <el-button
          type="primary"
          class="install-button"
          :loading="installing"
          :disabled="!update"
          @click="$emit('install')"
        >
          <el-icon v-if="!installing"><Download /></el-icon>
          {{ installing ? "正在更新" : "立即更新" }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.update-body {
  position: relative;
  padding: 30px 30px 8px;
}

.dialog-close {
  position: absolute;
  top: 14px;
  right: 14px;
  display: grid;
  width: 28px;
  height: 28px;
  padding: 0;
  font-size: 14px;
  color: #98a2b3;
  cursor: pointer;
  background: #f8fafc;
  border: 1px solid #e4e7ec;
  border-radius: 8px;
  transition:
    color 150ms ease,
    background 150ms ease;
  place-items: center;
}

.dialog-close:hover:not(:disabled) {
  color: #344054;
  background: #f2f4f7;
}

.dialog-close:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.update-mark {
  position: relative;
  display: grid;
  width: 54px;
  height: 54px;
  margin-bottom: 17px;
  font-size: 23px;
  color: #ecfdf9;
  background: linear-gradient(145deg, #14b8a6, #0f766e);
  border-radius: 16px;
  box-shadow: 0 12px 26px rgba(15, 118, 110, 0.25);
  place-items: center;
}

.mark-ring {
  position: absolute;
  inset: -6px;
  border: 1px solid rgba(15, 118, 110, 0.16);
  border-radius: 19px;
}

.eyebrow {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 700;
  letter-spacing: 2px;
  color: #0f766e;
}

.update-body h2 {
  margin: 6px 0 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 21px;
  font-weight: 650;
  letter-spacing: -0.4px;
  color: #101828;
}

.lead {
  max-width: 420px;
  margin: 7px 0 18px;
  font-size: 11.5px;
  line-height: 1.7;
  color: #667085;
}

.version-track {
  display: flex;
  gap: 11px;
  align-items: center;
  padding: 13px 15px;
  background: #f8fafc;
  border: 1px solid #e4e7ec;
  border-radius: 11px;
}

.version-block {
  display: flex;
  flex-direction: column;
  gap: 3px;
}

.version-block span {
  font-size: 9px;
  color: #98a2b3;
}

.version-block strong {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 15px;
  font-weight: 650;
  color: #475467;
  font-variant-numeric: tabular-nums;
}

.version-block.next strong {
  color: #0f766e;
}

.version-arrow {
  font-size: 14px;
  color: #b8c1cc;
}

.release-date {
  margin-left: auto;
  font-size: 9px;
  color: #98a2b3;
}

.notes-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin: 18px 1px 7px;
  font-size: 10px;
  font-weight: 650;
  color: #475467;
}

.notes-hint {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 500;
  color: #b0b8c4;
}

.release-notes {
  max-height: 146px;
  padding: 11px 13px;
  overflow-y: auto;
  font-size: 10px;
  line-height: 1.72;
  color: #667085;
  white-space: pre-wrap;
  background: #fcfcfd;
  border: 1px solid #eef0f3;
  border-radius: 9px;
}

.install-progress {
  padding: 14px 0 5px;
}

.progress-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 7px;
  font-size: 10px;
  font-weight: 600;
  color: #344054;
}

.progress-size {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  font-weight: 500;
  color: #98a2b3;
  font-variant-numeric: tabular-nums;
}

.progress-note {
  margin: 7px 0 0;
  font-size: 9px;
  color: #98a2b3;
}

.update-note {
  display: flex;
  gap: 7px;
  align-items: flex-start;
  padding: 11px 12px;
  margin-top: 12px;
  font-size: 9.5px;
  line-height: 1.6;
  color: #667085;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 8px;
}

.update-note .el-icon {
  flex: 0 0 auto;
  margin-top: 1px;
  color: #0f766e;
}

.update-footer {
  display: flex;
  gap: 9px;
  justify-content: flex-end;
  padding: 0 30px 24px;
}

.later-button,
.install-button {
  height: 34px;
  padding: 0 15px;
  font-size: 11.5px;
  border-radius: 9px;
}

.install-button {
  box-shadow: 0 8px 18px rgba(15, 118, 110, 0.2);
}

@media (max-width: 580px) {
  .update-body {
    padding: 24px 21px 6px;
  }

  .update-footer {
    padding: 0 21px 20px;
  }
}
</style>

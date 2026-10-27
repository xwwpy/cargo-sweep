<script setup lang="ts">
import { computed } from "vue";
import { Close, Delete, InfoFilled, WarningFilled } from "@element-plus/icons-vue";
import type { TargetInfo } from "../types";
import { formatBytes, formatCount } from "../lib/format";

const props = defineProps<{
  modelValue: boolean;
  targets: TargetInfo[];
  loading: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
  confirm: [];
}>();

const visible = computed({
  get: () => props.modelValue,
  set: (value: boolean) => emit("update:modelValue", value),
});

const totalBytes = computed(() =>
  props.targets.reduce((total, target) => total + target.sizeBytes, 0),
);

const totalFiles = computed(() =>
  props.targets.reduce((total, target) => total + target.fileCount, 0),
);

const previewTargets = computed(() => props.targets.slice(0, 4));
const overflowCount = computed(() => Math.max(0, props.targets.length - 4));

function close() {
  if (!props.loading) {
    visible.value = false;
  }
}

function handleBeforeClose(done: () => void) {
  if (!props.loading) {
    done();
  }
}
</script>

<template>
  <el-dialog
    v-model="visible"
    class="cleanup-dialog"
    width="500px"
    align-center
    append-to-body
    :show-close="false"
    :close-on-click-modal="!loading"
    :close-on-press-escape="!loading"
    :before-close="handleBeforeClose"
  >
    <div class="dialog-body">
      <button
        class="dialog-close"
        type="button"
        aria-label="关闭"
        :disabled="loading"
        @click="close"
      >
        <el-icon><Close /></el-icon>
      </button>

      <div class="warning-mark">
        <span class="warning-ring" />
        <el-icon><WarningFilled /></el-icon>
      </div>

      <h2>确认清理这些构建缓存？</h2>
      <p class="dialog-lead">
        以下 target 目录将被永久删除。源码不受影响，但目录内的构建产物无法恢复。
      </p>

      <div class="delete-summary">
        <div class="summary-block">
          <span class="summary-label">目录数量</span>
          <strong>{{ formatCount(targets.length) }}</strong>
          <span class="summary-unit">个</span>
        </div>
        <div class="summary-divider" />
        <div class="summary-block">
          <span class="summary-label">预计释放</span>
          <strong>{{ formatBytes(totalBytes) }}</strong>
        </div>
        <div class="summary-divider" />
        <div class="summary-block">
          <span class="summary-label">涉及文件</span>
          <strong>{{ formatCount(totalFiles) }}</strong>
          <span class="summary-unit">个</span>
        </div>
      </div>

      <div class="target-preview">
        <div v-for="target in previewTargets" :key="target.path" class="preview-row">
          <span class="preview-name">{{ target.projectName }}</span>
          <span class="preview-path">{{ target.path }}</span>
          <span class="preview-size">{{ formatBytes(target.sizeBytes) }}</span>
        </div>
        <div v-if="overflowCount > 0" class="preview-more">
          还有 {{ overflowCount }} 个 target 目录
        </div>
      </div>

      <div class="safety-note">
        <el-icon><InfoFilled /></el-icon>
        <span>只会删除校验通过的 Cargo target，Cargo.toml、源码和 Git 记录都不会被修改。</span>
      </div>
    </div>

    <template #footer>
      <div class="dialog-footer">
        <el-button class="cancel-button" :disabled="loading" @click="close">取消</el-button>
        <el-button
          type="danger"
          class="confirm-button"
          :loading="loading"
          @click="$emit('confirm')"
        >
          <el-icon v-if="!loading"><Delete /></el-icon>
          {{ loading ? "正在清理…" : "永久删除" }}
        </el-button>
      </div>
    </template>
  </el-dialog>
</template>

<style scoped>
.dialog-body {
  position: relative;
  padding: 30px 30px 6px;
}

.dialog-close {
  position: absolute;
  top: 13px;
  right: 13px;
  z-index: 2;
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
  opacity: 0.5;
}

.warning-mark {
  position: relative;
  display: grid;
  width: 52px;
  height: 52px;
  margin-bottom: 17px;
  font-size: 22px;
  color: #d92d20;
  background: linear-gradient(145deg, #fef3f2, #fee4e2);
  border: 1px solid #fecdca;
  border-radius: 15px;
  place-items: center;
}

.warning-ring {
  position: absolute;
  inset: -5px;
  border: 1px solid rgba(217, 45, 32, 0.12);
  border-radius: 18px;
}

.dialog-body h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 21px;
  font-weight: 650;
  letter-spacing: -0.4px;
  color: #101828;
}

.dialog-lead {
  max-width: 410px;
  margin: 8px 0 20px;
  font-size: 11.5px;
  line-height: 1.75;
  color: #667085;
}

.delete-summary {
  display: flex;
  gap: 9px;
  align-items: stretch;
  padding: 13px 15px;
  background: #f8fafc;
  border: 1px solid #e4e7ec;
  border-radius: 12px;
}

.summary-block {
  display: flex;
  flex: 1;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}

.summary-label {
  font-size: 9.5px;
  color: #98a2b3;
}

.summary-block strong {
  overflow: hidden;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 17px;
  font-weight: 600;
  color: #101828;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
}

.summary-unit {
  display: none;
}

.summary-divider {
  width: 1px;
  background: #e4e7ec;
}

.target-preview {
  padding: 7px 0;
  margin: 12px 0;
  border-top: 1px solid #eaecf0;
  border-bottom: 1px solid #eaecf0;
}

.preview-row {
  display: grid;
  grid-template-columns: 110px minmax(0, 1fr) auto;
  gap: 8px;
  align-items: center;
  padding: 6px 2px;
}

.preview-name {
  overflow: hidden;
  font-size: 10.5px;
  font-weight: 600;
  color: #344054;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-path {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #98a2b3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.preview-size {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9.5px;
  color: #0f766e;
  font-variant-numeric: tabular-nums;
}

.preview-more {
  padding: 5px 2px 2px;
  font-size: 9.5px;
  color: #667085;
}

.safety-note {
  display: flex;
  gap: 7px;
  align-items: flex-start;
  padding: 11px 12px;
  font-size: 9.5px;
  line-height: 1.65;
  color: #667085;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 9px;
}

.safety-note .el-icon {
  flex: 0 0 auto;
  margin-top: 1px;
  color: #0f766e;
}

.dialog-footer {
  display: flex;
  gap: 9px;
  justify-content: flex-end;
  padding: 0 30px 24px;
}

.cancel-button {
  height: 34px;
  padding: 0 15px;
  font-size: 11.5px;
  border-radius: 9px;
}

.confirm-button {
  height: 34px;
  padding: 0 16px;
  font-size: 11.5px;
  border-radius: 9px;
  box-shadow: 0 8px 18px rgba(217, 45, 32, 0.2);
}

@media (max-width: 560px) {
  .dialog-body {
    padding: 24px 21px 4px;
  }

  .dialog-footer {
    padding: 0 21px 20px;
  }

  .delete-summary {
    gap: 5px;
    padding: 11px;
  }

  .preview-row {
    grid-template-columns: 92px minmax(0, 1fr);
  }

  .preview-size {
    display: none;
  }
}
</style>

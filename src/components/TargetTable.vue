<script setup lang="ts">
import { computed, ref } from "vue";
import {
  Delete,
  DocumentCopy,
  FolderOpened,
  Lock,
  Search,
} from "@element-plus/icons-vue";
import { ElMessage } from "element-plus";
import type { ScanProgress, TargetInfo } from "../types";
import {
  basename,
  formatBytes,
  formatCount,
  formatDateTime,
  formatRelativeTime,
  splitBytes,
} from "../lib/format";

const props = defineProps<{
  targets: TargetInfo[];
  totalTargets: number;
  scanning: boolean;
  loading: boolean;
  deleting: boolean;
  search: string;
  selectedCount: number;
  selectedBytes: number;
  progress: ScanProgress | null;
}>();

const emit = defineEmits<{
  "update:search": [value: string];
  "selection-change": [rows: TargetInfo[]];
  "request-delete": [rows: TargetInfo[]];
  "request-delete-selected": [];
  "request-delete-all": [];
  "open-location": [target: TargetInfo];
  add: [];
}>();

const tableRef = ref<{ clearSelection: () => void } | null>(null);

const localSearch = computed({
  get: () => props.search,
  set: (value: string) => emit("update:search", value),
});

const maxSize = computed(() =>
  Math.max(1, ...props.targets.map((target) => target.sizeBytes)),
);

const scanTitle = computed(() => {
  if (props.loading) {
    return "正在准备工作区";
  }
  if (props.progress?.stage === "measuring") {
    return `正在计算 ${props.progress.found} 个 target 的空间`;
  }
  if (props.progress?.stage === "discovering") {
    return "正在发现 Cargo target";
  }
  return "正在扫描项目目录";
});

const scanDetail = computed(() => {
  if (props.loading) {
    return "读取扫描范围与本地数据";
  }

  const progress = props.progress;
  if (!progress) {
    return "扫描即将开始";
  }

  if (progress.stage === "measuring") {
    return "正在统计文件数量与占用空间，较大的目录可能需要几秒";
  }

  if (progress.root) {
    const current = Math.min(progress.rootsScanned + 1, progress.rootsTotal);
    return `${basename(progress.root)} · 第 ${current} / ${progress.rootsTotal} 个根目录`;
  }

  return "正在查找包含 Cargo.toml 的 target 目录";
});

function canSelect(): boolean {
  return !props.scanning && !props.deleting;
}

function handleSelection(rows: TargetInfo[]) {
  emit("selection-change", rows);
}

function requestDelete(row: TargetInfo) {
  emit("request-delete", [row]);
}

async function copyPath(path: string) {
  try {
    await navigator.clipboard.writeText(path);
    ElMessage.success("路径已复制");
  } catch {
    const textarea = document.createElement("textarea");
    textarea.value = path;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    const copied = document.execCommand("copy");
    document.body.removeChild(textarea);
    if (copied) {
      ElMessage.success("路径已复制");
    } else {
      ElMessage.error("复制失败，请手动选择路径");
    }
  }
}

defineExpose({
  clearSelection() {
    tableRef.value?.clearSelection();
  },
});
</script>

<template>
  <section class="targets-card" aria-label="Target 目录列表">
    <div class="toolbar">
      <el-input
        v-model="localSearch"
        class="search-field"
        :disabled="scanning || loading"
        clearable
        placeholder="搜索项目或路径"
      >
        <template #prefix>
          <el-icon><Search /></el-icon>
        </template>
      </el-input>

      <transition name="pill">
        <div v-if="selectedCount > 0" class="selection-pill">
          <span class="selection-count">{{ selectedCount }}</span>
          <span>已选</span>
          <i />
          <span class="selection-size">{{ formatBytes(selectedBytes) }}</span>
        </div>
      </transition>

      <div class="toolbar-spacer" />

      <el-button
        class="clean-all-button"
        :disabled="totalTargets === 0 || scanning || loading || deleting"
        @click="$emit('request-delete-all')"
      >
        <el-icon><Delete /></el-icon>
        一键清理
      </el-button>

      <el-button
        type="primary"
        class="delete-selected-button"
        :disabled="selectedCount === 0 || scanning || loading || deleting"
        @click="$emit('request-delete-selected')"
      >
        <el-icon><Delete /></el-icon>
        删除所选
      </el-button>
    </div>

    <div class="table-stage" :class="{ 'is-busy': scanning || loading }">
      <el-table
        ref="tableRef"
        :data="targets"
        row-key="path"
        :max-height="472"
        :selectable="canSelect"
        class="target-table"
        @selection-change="handleSelection"
      >
        <el-table-column type="selection" width="44" :selectable="canSelect" />

        <el-table-column label="项目 / 路径" min-width="250">
          <template #default="{ row }">
            <div class="project-cell">
              <div class="target-glyph" aria-hidden="true" />
              <div class="project-copy">
                <span class="project-name">{{ row.projectName }}</span>
                <button
                  class="project-path"
                  type="button"
                  :title="`复制路径：${row.path}`"
                  @click="copyPath(row.path)"
                >
                  <span>{{ row.path }}</span>
                  <el-icon class="copy-icon"><DocumentCopy /></el-icon>
                </button>
              </div>
            </div>
          </template>
        </el-table-column>

        <el-table-column label="占用空间" width="122" align="right">
          <template #default="{ row }">
            <div class="size-cell">
              <span class="size-value">{{ splitBytes(row.sizeBytes).value }}</span>
              <span class="size-unit">{{ splitBytes(row.sizeBytes).unit }}</span>
            </div>
            <div class="size-track">
              <span
                class="size-fill"
                :style="{ width: `${Math.max(5, (row.sizeBytes / maxSize) * 100)}%` }"
              />
            </div>
          </template>
        </el-table-column>

        <el-table-column label="文件数" width="86" align="right">
          <template #default="{ row }">
            <span class="muted-number">{{ formatCount(row.fileCount) }}</span>
          </template>
        </el-table-column>

        <el-table-column label="最近构建" width="104" align="right">
          <template #default="{ row }">
            <el-tooltip
              v-if="row.modifiedMs"
              :content="formatDateTime(row.modifiedMs)"
              placement="top"
              :show-after="400"
            >
              <span class="muted-time">{{ formatRelativeTime(row.modifiedMs) }}</span>
            </el-tooltip>
            <span v-else class="muted-time">未知</span>
          </template>
        </el-table-column>

        <el-table-column label="操作" width="96" align="right">
          <template #default="{ row }">
            <div class="row-actions">
              <el-tooltip content="打开 target 目录" placement="top" :show-after="350">
                <el-button
                  class="row-open"
                  text
                  circle
                  :disabled="scanning || deleting"
                  aria-label="打开 target 目录"
                  @click.stop="$emit('open-location', row)"
                >
                  <el-icon><FolderOpened /></el-icon>
                </el-button>
              </el-tooltip>
              <el-button
                class="row-delete"
                link
                type="danger"
                :disabled="scanning || deleting"
                @click.stop="requestDelete(row)"
              >
                清理
              </el-button>
            </div>
          </template>
        </el-table-column>

        <template #empty>
          <div class="empty-state">
            <template v-if="search">
              <div class="empty-symbol search-empty">
                <el-icon><Search /></el-icon>
              </div>
              <h3>没有匹配的项目</h3>
              <p>换一个关键词试试，或清空搜索查看全部 target。</p>
              <el-button class="empty-button" @click="localSearch = ''">清空搜索</el-button>
            </template>
            <template v-else>
              <div class="empty-symbol">
                <span class="empty-orbit orbit-a" />
                <span class="empty-orbit orbit-b" />
                <span class="empty-core" />
              </div>
              <h3>还没有发现 target 目录</h3>
              <p>添加 Rust 项目所在的文件夹，我们会自动找出其中的构建缓存。</p>
              <el-button type="primary" class="empty-button" @click="$emit('add')">
                <el-icon><FolderOpened /></el-icon>
                添加扫描目录
              </el-button>
            </template>
          </div>
        </template>
      </el-table>

      <transition name="overlay">
        <div v-if="scanning || loading" class="scan-overlay" aria-live="polite">
          <div class="scan-visual" aria-hidden="true">
            <span class="scan-ring ring-a" />
            <span class="scan-ring ring-b" />
            <span class="scan-ring ring-c" />
            <span class="scan-core" />
          </div>
          <h3>{{ scanTitle }}</h3>
          <p>{{ scanDetail }}</p>
          <div class="scan-progress">
            <el-progress
              :percentage="100"
              :show-text="false"
              :indeterminate="true"
              :duration="1.6"
              :stroke-width="3"
              color="#0f766e"
            />
          </div>
          <span v-if="progress && progress.found > 0" class="scan-found">
            已发现 {{ progress.found }} 个 target
          </span>
        </div>
      </transition>
    </div>

    <footer class="table-footer">
      <span class="footer-security">
        <el-icon><Lock /></el-icon>
        仅删除包含 Cargo.toml 的 target 目录
      </span>
      <span class="footer-count">
        {{ totalTargets }} 个项目 · 显示 {{ targets.length }} 个
      </span>
    </footer>
  </section>
</template>

<style scoped>
.targets-card {
  overflow: hidden;
  background: #ffffff;
  border: 1px solid rgba(16, 24, 40, 0.08);
  border-radius: 18px;
  box-shadow:
    0 1px 0 #ffffff inset,
    0 18px 44px rgba(16, 24, 40, 0.04);
  animation: rise 600ms 140ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.toolbar {
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 14px 16px;
  background: rgba(248, 250, 252, 0.9);
  border-bottom: 1px solid #eaecf0;
}

.search-field {
  width: 276px;
}

.search-field :deep(.el-input__wrapper) {
  min-height: 34px;
  padding: 0 11px;
  background: #fff;
  border-radius: 9px;
  box-shadow: 0 0 0 1px #dfe4ea inset;
  transition:
    box-shadow 160ms ease,
    background 160ms ease;
}

.search-field :deep(.el-input__wrapper.is-focus) {
  background: #fff;
  box-shadow:
    0 0 0 1px #0f766e inset,
    0 0 0 3px rgba(15, 118, 110, 0.09);
}

.search-field :deep(.el-input__inner) {
  font-size: 12px;
  color: #344054;
}

.search-field :deep(.el-input__inner::placeholder) {
  color: #98a2b3;
}

.search-field :deep(.el-input__prefix) {
  font-size: 13px;
  color: #98a2b3;
}

.selection-pill {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 6px 9px 6px 6px;
  font-size: 10.5px;
  color: #475467;
  white-space: nowrap;
  background: #edf9f7;
  border: 1px solid #cdebe6;
  border-radius: 8px;
}

.selection-count {
  display: grid;
  min-width: 20px;
  height: 20px;
  padding: 0 5px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 10px;
  font-weight: 600;
  color: #fff;
  background: #0f766e;
  border-radius: 5px;
  place-items: center;
}

.selection-pill i {
  width: 2px;
  height: 2px;
  background: #9fb4b0;
  border-radius: 50%;
}

.selection-size {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-weight: 600;
  color: #0f766e;
}

.toolbar-spacer {
  flex: 1;
}

.clean-all-button,
.delete-selected-button {
  height: 34px;
  padding: 0 12px;
  font-size: 11.5px;
  border-radius: 9px;
}

.clean-all-button {
  color: #475467;
  background: #fff;
  border-color: #e4e7ec;
}

.clean-all-button:hover:not(:disabled) {
  color: #0f766e;
  background: #f7fffd;
  border-color: #a7e5da;
}

.delete-selected-button {
  box-shadow: 0 7px 16px rgba(15, 118, 110, 0.18);
}

.table-stage {
  position: relative;
}

.table-stage.is-busy :deep(.el-table__empty-block) {
  visibility: hidden;
}

.target-table {
  width: 100%;
  --el-table-header-bg-color: #f8fafc;
  --el-table-tr-bg-color: transparent;
  --el-table-row-hover-bg-color: #f0fdfa;
  --el-table-border-color: transparent;
  --el-table-header-text-color: #667085;
  --el-table-text-color: #344054;
}

.target-table :deep(.el-table__inner-wrapper::before) {
  display: none;
}

.target-table :deep(th.el-table__cell) {
  height: 38px;
  padding: 0;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.45px;
  background: #f8fafc;
  border-bottom: 1px solid #eaecf0;
}

.target-table :deep(td.el-table__cell) {
  height: 67px;
  padding: 0;
  border-bottom: 1px solid #f2f4f7;
}

.target-table :deep(.el-table__row:last-child td.el-table__cell) {
  border-bottom: 0;
}

.target-table :deep(.el-table__row.current-row > td.el-table__cell),
.target-table :deep(.el-table__row:hover > td.el-table__cell) {
  background: #f0fdfa;
}

.target-table :deep(.el-checkbox__inner) {
  width: 14px;
  height: 14px;
  background: #fff;
  border-color: #d0d5dd;
  border-radius: 4px;
}

.target-table :deep(.el-checkbox__input.is-checked .el-checkbox__inner),
.target-table :deep(.el-checkbox__input.is-indeterminate .el-checkbox__inner) {
  background: #0f766e;
  border-color: #0f766e;
}

.target-table :deep(.el-checkbox__inner::after) {
  width: 3px;
  height: 7px;
}

.target-table :deep(.cell) {
  padding: 0 12px;
}

.target-table :deep(.el-table__empty-block) {
  min-height: 306px;
}

.project-cell {
  display: flex;
  gap: 11px;
  align-items: center;
  min-width: 0;
}

.target-glyph {
  position: relative;
  display: grid;
  flex: 0 0 33px;
  width: 33px;
  height: 33px;
  background:
    linear-gradient(140deg, rgba(255, 255, 255, 0.65), transparent),
    #ecfdf9;
  border: 1px solid #d1f5ec;
  border-radius: 10px;
  place-items: center;
}

.target-glyph::before {
  width: 15px;
  height: 15px;
  content: "";
  border: 1.5px solid #0f766e;
  border-radius: 50%;
  box-shadow: 0 0 0 3px rgba(15, 118, 110, 0.08);
}

.target-glyph::after {
  position: absolute;
  width: 4px;
  height: 4px;
  content: "";
  background: #0f766e;
  border-radius: 50%;
}

.project-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.project-name {
  overflow: hidden;
  font-size: 12.5px;
  font-weight: 650;
  color: #101828;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.project-path {
  display: flex;
  gap: 5px;
  align-items: center;
  max-width: 100%;
  padding: 0;
  margin-top: 3px;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9.5px;
  color: #98a2b3;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: 0;
}

.project-path span {
  overflow: hidden;
  text-overflow: ellipsis;
}

.project-path:hover {
  color: #0f766e;
}

.copy-icon {
  flex: 0 0 auto;
  font-size: 10px;
  opacity: 0;
  transition: opacity 140ms ease;
}

.project-path:hover .copy-icon {
  opacity: 1;
}

.size-cell {
  display: flex;
  gap: 3px;
  align-items: baseline;
  justify-content: flex-end;
  font-variant-numeric: tabular-nums;
}

.size-value {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 13.5px;
  font-weight: 600;
  color: #101828;
}

.size-unit {
  font-size: 9px;
  font-weight: 600;
  color: #98a2b3;
}

.size-track {
  width: 58px;
  height: 3px;
  margin: 6px 0 0 auto;
  overflow: hidden;
  background: #e9eef2;
  border-radius: 4px;
}

.size-fill {
  display: block;
  height: 100%;
  background: linear-gradient(90deg, #2dd4bf, #0f766e);
  border-radius: inherit;
}

.muted-number,
.muted-time {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: #667085;
}

.row-actions {
  display: flex;
  gap: 2px;
  align-items: center;
  justify-content: flex-end;
}

.row-open {
  width: 24px;
  height: 24px;
  padding: 0;
  font-size: 12px;
  color: #98a2b3;
  border-radius: 7px;
}

.row-open:not(.is-disabled):hover {
  color: #0f766e;
  background: #edf9f7;
}

.row-delete {
  height: 26px;
  padding: 0 6px;
  font-size: 10.5px;
  font-weight: 600;
  border-radius: 7px;
}

.row-delete:not(.is-disabled):hover {
  background: rgba(217, 45, 32, 0.06);
}

.table-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 17px;
  background: #f8fafc;
  border-top: 1px solid #eaecf0;
}

.footer-security,
.footer-count {
  display: flex;
  gap: 5px;
  align-items: center;
  font-size: 9.5px;
  color: #98a2b3;
}

.footer-security .el-icon {
  font-size: 10px;
  color: #7a8a9e;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 20px 54px;
  text-align: center;
}

.empty-symbol {
  position: relative;
  display: grid;
  width: 64px;
  height: 64px;
  margin-bottom: 18px;
  background: #f7fbfa;
  border: 1px solid #e3efec;
  border-radius: 20px;
  place-items: center;
}

.empty-symbol.search-empty {
  font-size: 23px;
  color: #0f766e;
}

.empty-orbit {
  position: absolute;
  border: 1px solid #6fcabb;
  border-radius: 50%;
}

.orbit-a {
  width: 30px;
  height: 30px;
  opacity: 0.5;
}

.orbit-b {
  width: 20px;
  height: 20px;
  border-color: #14b8a6;
  opacity: 0.7;
}

.empty-core {
  width: 7px;
  height: 7px;
  background: #0f766e;
  border-radius: 50%;
  box-shadow: 0 0 0 4px rgba(15, 118, 110, 0.08);
}

.empty-state h3 {
  margin: 0;
  font-size: 14px;
  font-weight: 650;
  color: #101828;
}

.empty-state p {
  max-width: 330px;
  margin: 7px 0 16px;
  font-size: 11px;
  line-height: 1.7;
  color: #667085;
}

.empty-button {
  height: 32px;
  padding: 0 13px;
  font-size: 11px;
  border-radius: 8px;
}

.scan-overlay {
  position: absolute;
  inset: 0;
  z-index: 5;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 360px;
  background: rgba(248, 250, 252, 0.92);
  backdrop-filter: blur(4px);
}

.scan-visual {
  position: relative;
  display: grid;
  width: 72px;
  height: 72px;
  margin-bottom: 20px;
  place-items: center;
}

.scan-ring {
  position: absolute;
  border: 1px solid rgba(15, 118, 110, 0.24);
  border-radius: 50%;
}

.ring-a {
  width: 70px;
  height: 70px;
  border-style: dashed;
  animation: spin 8s linear infinite;
}

.ring-b {
  width: 49px;
  height: 49px;
  border-color: rgba(15, 118, 110, 0.42);
  animation: spin-reverse 5.5s linear infinite;
}

.ring-c {
  width: 30px;
  height: 30px;
  border-color: rgba(15, 118, 110, 0.18);
}

.scan-core {
  width: 10px;
  height: 10px;
  background: #0f766e;
  border-radius: 50%;
  box-shadow: 0 0 0 6px rgba(15, 118, 110, 0.09);
  animation: pulse 1.6s ease-in-out infinite;
}

.scan-overlay h3 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 15px;
  font-weight: 650;
  color: #101828;
}

.scan-overlay p {
  max-width: 400px;
  margin: 7px 20px 14px;
  overflow: hidden;
  font-size: 10.5px;
  color: #667085;
  text-align: center;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.scan-progress {
  width: min(260px, 62%);
}

.scan-progress :deep(.el-progress-bar__outer) {
  background: #e9eef2;
  border-radius: 4px;
}

.scan-found {
  padding: 4px 8px;
  margin-top: 11px;
  font-size: 9.5px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 6px;
}

.pill-enter-active,
.pill-leave-active {
  transition:
    opacity 180ms ease,
    transform 180ms ease;
}

.pill-enter-from,
.pill-leave-to {
  opacity: 0;
  transform: translateX(-4px);
}

.overlay-enter-active,
.overlay-leave-active {
  transition: opacity 220ms ease;
}

.overlay-enter-from,
.overlay-leave-to {
  opacity: 0;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes spin-reverse {
  to {
    transform: rotate(-360deg);
  }
}

@keyframes pulse {
  0%,
  100% {
    box-shadow: 0 0 0 5px rgba(15, 118, 110, 0.07);
    transform: scale(0.92);
  }
  50% {
    box-shadow: 0 0 0 9px rgba(15, 118, 110, 0.03);
    transform: scale(1);
  }
}

@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (max-width: 1180px) {
  .search-field {
    width: 220px;
  }

  .selection-size {
    display: none;
  }

  .selection-pill i {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .targets-card,
  .scan-ring,
  .scan-core {
    animation: none;
  }
}
</style>

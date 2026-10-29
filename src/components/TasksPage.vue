<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  CircleCheck,
  Close,
  Delete,
  Loading,
  Refresh,
  WarningFilled,
} from "@element-plus/icons-vue";
import {
  activeTaskCount,
  backgroundTasks,
  cancelTask,
  cleanupTask,
  clearFinishedTasks,
  removeTask,
  type BackgroundTask,
} from "../composables/useTasks";

const now = ref(Date.now());
let clock: number | undefined;
onMounted(() => {
  clock = window.setInterval(() => {
    now.value = Date.now();
  }, 1000);
});
onUnmounted(() => {
  window.clearInterval(clock);
});

const activeCount = activeTaskCount;
const finishedCount = computed(
  () =>
    backgroundTasks.value.filter(
      (task) => task.status !== "running" && task.status !== "cancelling",
    ).length,
);

function statusLabel(task: BackgroundTask): string {
  switch (task.status) {
    case "running":
      return "进行中";
    case "cancelling":
      return "取消中";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    case "cancelled":
      return "已取消";
    default:
      return task.status;
  }
}

function kindLabel(task: BackgroundTask): string {
  switch (task.kind) {
    case "delete":
      return "空间清理";
    case "toolchain":
      return "工具链安装 / 更新";
    case "rustup":
      return "rustup 安装";
    case "uninstall":
      return "工具链删除";
    default:
      return "后台任务";
  }
}

function componentPhaseLabel(phase: string): string {
  switch (phase) {
    case "downloading":
      return "下载中";
    case "installing":
      return "安装中";
    case "done":
      return "完成";
    default:
      return "等待";
  }
}

function progressLabel(task: BackgroundTask): string {
  if (task.progressMode === "indeterminate") {
    return isActive(task)
      ? task.status === "cancelling"
        ? "取消中"
        : "处理中"
      : statusLabel(task);
  }
  return `${task.percent}%`;
}

function elapsedLabel(task: BackgroundTask): string {
  const end = task.finishedAt ?? now.value;
  const seconds = Math.max(0, Math.round((end - task.startedAt) / 1000));
  if (seconds < 60) {
    return `${seconds}s`;
  }
  return `${Math.floor(seconds / 60)}m ${seconds % 60}s`;
}

async function requestCancel(task: BackgroundTask) {
  try {
    await ElMessageBox.confirm(
      `取消「${task.title}」并清理本次任务的中间文件？`,
      "取消后台任务",
      {
        type: "warning",
        confirmButtonText: "取消任务",
        cancelButtonText: "继续运行",
      },
    );
  } catch {
    return;
  }
  await cancelTask(task.id);
}

async function clean(task: BackgroundTask) {
  await cleanupTask(task.id);
  if (task.cleanupMessage) {
    ElMessage.info(task.cleanupMessage);
  }
}

function remove(task: BackgroundTask) {
  removeTask(task.id);
}

function clearFinished() {
  clearFinishedTasks();
}

function isActive(task: BackgroundTask): boolean {
  return task.status === "running" || task.status === "cancelling";
}
</script>

<template>
  <section class="tasks-page">
    <header class="tasks-hero">
      <div>
        <span class="tasks-kicker"><i /> BACKGROUND TASKS</span>
        <h1>任务<span>中心</span></h1>
        <p>任务在后台执行，可以离开当前页面继续其他操作。</p>
      </div>
      <div class="tasks-summary">
        <span class="tasks-summary-number">{{ activeCount }}</span>
        <span>个任务进行中</span>
        <button
          v-if="finishedCount > 0"
          type="button"
          class="tasks-clear"
          @click="clearFinished"
        >
          清空已结束
        </button>
      </div>
    </header>

    <div v-if="backgroundTasks.length === 0" class="tasks-empty">
      <el-icon><CircleCheck /></el-icon>
      <h2>当前没有后台任务</h2>
      <p>清理 target 或安装工具链后，任务会出现在这里。</p>
    </div>

    <div v-else class="task-list">
      <article
        v-for="task in backgroundTasks"
        :key="task.id"
        class="task-card"
        :class="task.status"
      >
        <div class="task-card-head">
          <div class="task-heading">
            <span class="task-kind">{{ kindLabel(task) }}</span>
            <h2>{{ task.title }}</h2>
            <p>
              {{ task.detail }}
              <span v-if="task.error" class="task-error">{{ task.error }}</span>
            </p>
          </div>
          <span class="task-status" :class="task.status">
            <el-icon v-if="isActive(task)" class="spinning">
              <Loading />
            </el-icon>
            <el-icon v-else-if="task.status === 'completed'">
              <CircleCheck />
            </el-icon>
            <el-icon v-else-if="task.status === 'failed'">
              <WarningFilled />
            </el-icon>
            <el-icon v-else><Close /></el-icon>
            {{ statusLabel(task) }}
          </span>
        </div>

        <div class="task-progress-row">
          <div
            class="task-progress-track"
            :class="{
              indeterminate:
                task.progressMode === 'indeterminate' &&
                task.status === 'running',
              failed: task.status === 'failed',
            }"
          >
            <span
              :style="{
                width:
                  task.progressMode === 'determinate'
                    ? `${task.percent}%`
                    : '100%',
              }"
            />
          </div>
          <span class="task-progress-value">{{ progressLabel(task) }}</span>
          <span>{{ elapsedLabel(task) }}</span>
        </div>
        <p class="task-message">{{ task.message }}</p>

        <div v-if="task.components.length > 0" class="task-components">
          <div
            v-for="component in task.components"
            :key="component.name"
            class="task-component"
          >
            <span>{{ component.name }}</span>
            <div class="task-component-track">
              <span
                :class="{ indeterminate: component.indeterminate }"
                :style="{
                  width: component.indeterminate
                    ? '100%'
                    : `${component.percent}%`,
                }"
              />
            </div>
            <small>{{ componentPhaseLabel(component.phase) }}</small>
          </div>
        </div>

        <details v-if="task.output.length > 0" class="task-output">
          <summary>任务输出（{{ task.output.length }} 行）</summary>
          <pre>{{ task.output.join("\n") }}</pre>
        </details>

        <footer class="task-actions">
          <button
            v-if="isActive(task)"
            type="button"
            class="task-action danger"
            :disabled="task.status === 'cancelling'"
            @click="requestCancel(task)"
          >
            <el-icon><Close /></el-icon>
            {{ task.status === "cancelling" ? "取消中" : "取消任务" }}
          </button>
          <button
            v-if="!isActive(task)"
            type="button"
            class="task-action"
            :disabled="task.cleanupState === 'cleaning'"
            @click="clean(task)"
          >
            <el-icon><Refresh /></el-icon>
            {{
              task.cleanupState === "cleaning"
                ? "清理中"
                : task.cleanupState === "done"
                  ? "再次清理"
                  : "清理中间文件"
            }}
          </button>
          <span v-if="task.cleanupMessage" class="task-clean-message">
            {{ task.cleanupMessage }}
          </span>
          <button
            v-if="!isActive(task)"
            type="button"
            class="task-action subtle"
            @click="remove(task)"
          >
            <el-icon><Delete /></el-icon>
            移除记录
          </button>
        </footer>
      </article>
    </div>
  </section>
</template>

<style scoped>
.tasks-page {
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding-bottom: 20px;
}

.tasks-hero {
  display: flex;
  gap: 20px;
  align-items: flex-end;
  justify-content: space-between;
  padding: 22px 2px 8px;
}

.tasks-kicker {
  display: flex;
  gap: 8px;
  align-items: center;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 700;
  letter-spacing: 2.4px;
  color: var(--accent-1);
}

.tasks-kicker i {
  width: 20px;
  height: 2px;
  background: var(--accent-1);
  border-radius: 2px;
}

.tasks-hero h1 {
  margin: 7px 0 5px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: clamp(30px, 3vw, 40px);
  font-weight: 780;
  letter-spacing: -1.8px;
  color: var(--app-ink);
}

.tasks-hero h1 span {
  color: var(--accent-1);
}

.tasks-hero p {
  margin: 0;
  font-size: 9px;
  color: var(--app-muted);
}

.tasks-summary {
  display: flex;
  gap: 7px;
  align-items: baseline;
  font-size: 9px;
  color: var(--app-muted);
}

.tasks-summary-number {
  font-size: 25px;
  font-weight: 760;
  color: var(--accent-1);
}

.tasks-clear {
  margin-left: 8px;
  padding: 5px 8px;
  font-size: 8px;
  color: var(--app-muted);
  cursor: pointer;
  background: transparent;
  border: 1px solid var(--surface-line);
  border-radius: 7px;
}

.tasks-clear:hover {
  color: var(--accent-1);
  border-color: var(--accent-1-line);
}

.tasks-empty {
  display: flex;
  flex-direction: column;
  gap: 7px;
  align-items: center;
  justify-content: center;
  min-height: 260px;
  color: var(--app-muted);
  text-align: center;
  background: var(--surface-strong);
  border: 1px dashed var(--surface-line);
  border-radius: 16px;
}

.tasks-empty .el-icon {
  font-size: 30px;
  color: var(--accent-1);
}

.tasks-empty h2 {
  margin: 0;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 15px;
  color: var(--app-ink);
}

.tasks-empty p {
  margin: 0;
  font-size: 9px;
}

.task-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.task-card {
  padding: 14px 16px;
  background: var(--surface-strong);
  border: 1px solid var(--surface-line);
  border-radius: 16px;
  box-shadow: var(--surface-shadow);
}

.task-card.running {
  border-color: var(--accent-1-soft-line);
}

.task-card.failed {
  border-color: rgba(220, 38, 38, 0.28);
}

.task-card-head {
  display: flex;
  gap: 14px;
  align-items: flex-start;
  justify-content: space-between;
}

.task-heading {
  min-width: 0;
}

.task-kind {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 7px;
  font-weight: 700;
  letter-spacing: 1px;
  color: var(--accent-1);
  text-transform: uppercase;
}

.task-heading h2 {
  margin: 3px 0 2px;
  overflow: hidden;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 13px;
  font-weight: 700;
  color: var(--app-ink);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-heading p {
  margin: 0;
  font-size: 8px;
  color: var(--app-muted);
}

.task-error {
  color: #dc2626;
}

.task-status {
  display: flex;
  flex-shrink: 0;
  gap: 4px;
  align-items: center;
  padding: 3px 7px;
  font-size: 8px;
  color: var(--app-muted);
  background: var(--app-bg);
  border: 1px solid var(--surface-line);
  border-radius: 999px;
}

.task-status.running,
.task-status.cancelling {
  color: var(--accent-1);
}

.task-status.completed {
  color: #0d9488;
}

.task-status.failed {
  color: #dc2626;
}

.task-progress-row {
  display: flex;
  gap: 8px;
  align-items: center;
  margin-top: 11px;
}

.task-progress-track {
  flex: 1;
  height: 7px;
  overflow: hidden;
  background: color-mix(in srgb, var(--accent-1) 12%, transparent);
  border-radius: 99px;
}

.task-progress-track > span {
  display: block;
  height: 100%;
  background: var(--primary-gradient);
  border-radius: inherit;
  transition: width 260ms ease;
}

.task-progress-track.indeterminate > span {
  width: 100%;
  background: linear-gradient(
    90deg,
    transparent 0%,
    var(--accent-1) 22%,
    var(--accent-2) 48%,
    var(--accent-1) 74%,
    transparent 100%
  );
  background-size: 220% 100%;
  animation: task-progress-flow 1.25s linear infinite;
}

.task-progress-track.failed > span {
  background: #dc2626;
}

.task-progress-row > span {
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--app-muted);
  font-variant-numeric: tabular-nums;
}

.task-progress-value {
  min-width: 40px;
  color: var(--accent-1) !important;
  font-weight: 650;
  text-align: right;
}

@keyframes task-progress-flow {
  from {
    background-position: 180% 0;
  }
  to {
    background-position: -80% 0;
  }
}

.task-message {
  margin: 5px 0 0;
  overflow: hidden;
  font-size: 8px;
  color: var(--app-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-components {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding-top: 8px;
  margin-top: 8px;
  border-top: 1px solid var(--surface-line);
}

.task-component {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr) 48px;
  gap: 7px;
  align-items: center;
}

.task-component > span {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: var(--app-ink);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-component-track {
  height: 4px;
  overflow: hidden;
  background: rgba(139, 92, 246, 0.12);
  border-radius: 99px;
}

.task-component-track > span {
  display: block;
  height: 100%;
  background: var(--accent-1);
  border-radius: inherit;
}

.task-component-track > span.indeterminate {
  background: linear-gradient(
    90deg,
    var(--accent-1-soft),
    var(--accent-1),
    var(--accent-1-soft)
  );
  background-size: 200% 100%;
  animation: task-shimmer 1.2s linear infinite;
}

.task-component small {
  font-size: 7px;
  color: var(--app-muted);
  text-align: right;
}

.task-output {
  margin-top: 9px;
  overflow: hidden;
  background: #0d0915;
  border: 1px solid rgba(168, 85, 247, 0.16);
  border-radius: 10px;
}

.task-output summary {
  padding: 6px 9px;
  font-size: 8px;
  color: #d8b4fe;
  cursor: pointer;
  background: rgba(168, 85, 247, 0.08);
}

.task-output pre {
  max-height: 170px;
  padding: 8px 10px;
  margin: 0;
  overflow: auto;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 7.5px;
  line-height: 1.6;
  color: #c4b5d7;
  white-space: pre-wrap;
  word-break: break-all;
}

.task-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  align-items: center;
  padding-top: 9px;
  margin-top: 9px;
  border-top: 1px solid var(--surface-line);
}

.task-action {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 5px 8px;
  font-size: 8px;
  color: var(--app-muted);
  cursor: pointer;
  background: transparent;
  border: 1px solid var(--surface-line);
  border-radius: 7px;
}

.task-action:hover:not(:disabled) {
  color: var(--accent-1);
  border-color: var(--accent-1-line);
}

.task-action.danger:hover:not(:disabled) {
  color: #dc2626;
  border-color: rgba(220, 38, 38, 0.3);
}

.task-action.subtle {
  margin-left: auto;
}

.task-action:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.task-clean-message {
  overflow: hidden;
  font-size: 7.5px;
  color: var(--app-muted);
  text-overflow: ellipsis;
  white-space: nowrap;
}

.spinning {
  animation: spinning 1.1s linear infinite;
}

@keyframes spinning {
  to {
    transform: rotate(360deg);
  }
}

@keyframes task-shimmer {
  from {
    background-position: 200% 0;
  }
  to {
    background-position: -20% 0;
  }
}

@media (max-width: 700px) {
  .tasks-hero {
    align-items: flex-start;
    flex-direction: column;
  }

  .task-heading h2 {
    white-space: normal;
  }
}
</style>

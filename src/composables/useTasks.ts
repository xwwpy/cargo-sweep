import { computed, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import {
  cancelBackgroundTask,
  cleanupTaskArtifacts,
  errorMessage,
  isTauriRuntime,
} from "../api";
import type { ComponentProgress, TaskProgressEvent } from "../types";

export type TaskKind =
  | "delete"
  | "toolchain"
  | "rustup"
  | "uninstall";
export type TaskStatus =
  | "running"
  | "cancelling"
  | "completed"
  | "failed"
  | "cancelled";
export type TaskPage = "targets" | "rust" | "tasks";

export interface BackgroundTask {
  id: string;
  kind: TaskKind;
  title: string;
  detail: string;
  page: TaskPage;
  status: TaskStatus;
  percent: number;
  message: string;
  phase: string;
  components: ComponentProgress[];
  speedBps: number;
  downloadedBytes: number;
  elapsedMs: number;
  output: string[];
  startedAt: number;
  finishedAt: number | null;
  cancelRequested: boolean;
  cleanupState: "idle" | "cleaning" | "done" | "failed";
  cleanupMessage: string;
  error: string | null;
}

interface StartTaskOptions<T> {
  kind: TaskKind;
  title: string;
  detail?: string;
  page?: TaskPage;
  run: (taskId: string) => Promise<T>;
  onFinished?: (result: T | null, error: unknown) => void;
  successMessage?: string;
}

const MAX_OUTPUT_LINES = 600;

export const backgroundTasks = ref<BackgroundTask[]>([]);

export const activeTasks = computed(() =>
  backgroundTasks.value.filter(
    (task) => task.status === "running" || task.status === "cancelling",
  ),
);

export const activeTaskCount = computed(() => activeTasks.value.length);

export const hasRunningToolchainTask = computed(() =>
  activeTasks.value.some(
    (task) =>
      task.kind === "toolchain" ||
      task.kind === "rustup" ||
      task.kind === "uninstall",
  ),
);

export const hasRunningDeleteTask = computed(() =>
  activeTasks.value.some((task) => task.kind === "delete"),
);

function createId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `task-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

function findTask(taskId: string): BackgroundTask | undefined {
  return backgroundTasks.value.find((task) => task.id === taskId);
}

function applyProgress(task: BackgroundTask, payload: TaskProgressEvent) {
  task.percent = Math.max(0, Math.min(100, payload.percent));
  if (payload.message) {
    task.message = payload.message;
  }
  task.phase = payload.phase || task.phase;
  task.components = payload.components ?? [];
  task.speedBps = payload.speedBps ?? 0;
  task.downloadedBytes = payload.downloadedBytes ?? 0;
  task.elapsedMs = payload.elapsedMs ?? task.elapsedMs;
  if (payload.line) {
    const line = payload.line.trimEnd();
    if (line) {
      task.output.push(line);
      if (task.output.length > MAX_OUTPUT_LINES) {
        task.output.splice(0, task.output.length - MAX_OUTPUT_LINES);
      }
    }
  }
}

if (isTauriRuntime()) {
  void listen<TaskProgressEvent>("task-progress", (event) => {
    const task = findTask(event.payload.taskId);
    if (task && (task.status === "running" || task.status === "cancelling")) {
      applyProgress(task, event.payload);
    }
  }).catch(() => {
    // Event bridge is unavailable in browser previews.
  });
}

export function startTask<T>(options: StartTaskOptions<T>): string {
  const taskId = createId();
  const task: BackgroundTask = {
    id: taskId,
    kind: options.kind,
    title: options.title,
    detail: options.detail ?? "",
    page: options.page ?? "tasks",
    status: "running",
    percent: 0,
    message: "任务已创建",
    phase: "queued",
    components: [],
    speedBps: 0,
    downloadedBytes: 0,
    elapsedMs: 0,
    output: [],
    startedAt: Date.now(),
    finishedAt: null,
    cancelRequested: false,
    cleanupState: "idle",
    cleanupMessage: "",
    error: null,
  };
  backgroundTasks.value.unshift(task);

  void options
    .run(taskId)
    .then((result) => {
      const current = findTask(taskId);
      if (!current) {
        return;
      }
      if (current.cancelRequested) {
        current.status = "cancelled";
        current.message = "任务已取消";
        current.percent = Math.min(current.percent, 99);
        void cleanupTask(taskId);
      } else {
        const failed =
          typeof result === "object" &&
          result !== null &&
          "success" in result &&
          result.success === false;
        current.status = failed ? "failed" : "completed";
        current.percent = failed ? Math.min(current.percent, 99) : 100;
        current.message = failed
          ? "任务结束，但命令返回了失败状态"
          : options.successMessage || "任务完成";
        current.phase = failed ? "failed" : "done";
      }
      current.finishedAt = Date.now();
      current.elapsedMs = Date.now() - current.startedAt;
      options.onFinished?.(result, null);
    })
    .catch((error) => {
      const current = findTask(taskId);
      if (!current) {
        return;
      }
      if (current.cancelRequested) {
        current.status = "cancelled";
        current.message = "任务已取消";
        void cleanupTask(taskId);
      } else {
        current.status = "failed";
        current.message = "任务失败";
        current.error = errorMessage(error);
        current.output.push(`[失败] ${current.error}`);
      }
      current.finishedAt = Date.now();
      current.elapsedMs = Date.now() - current.startedAt;
      options.onFinished?.(null, error);
    });
  return taskId;
}

export async function cancelTask(taskId: string): Promise<void> {
  const task = findTask(taskId);
  if (!task || (task.status !== "running" && task.status !== "cancelling")) {
    return;
  }
  task.cancelRequested = true;
  task.status = "cancelling";
  task.message = "正在取消任务";
  try {
    await cancelBackgroundTask(taskId);
  } catch (error) {
    task.error = errorMessage(error);
    task.message = "取消请求发送失败";
  }
}

export async function cleanupTask(
  taskId: string,
  automatic = false,
): Promise<void> {
  const task = findTask(taskId);
  if (!task || task.cleanupState === "cleaning") {
    return;
  }
  task.cleanupState = "cleaning";
  try {
    const report = await cleanupTaskArtifacts(task.startedAt, task.kind);
    task.cleanupState = "done";
    task.cleanupMessage = report.skipped
      ? report.message
      : `${report.message}（${report.removedFiles} 个文件）`;
  } catch (error) {
    task.cleanupState = "failed";
    task.cleanupMessage = `清理失败：${errorMessage(error)}`;
  }
  if (!automatic || task.cleanupState === "done") {
    task.message = task.cleanupMessage;
  }
}

export function removeTask(taskId: string): void {
  const task = findTask(taskId);
  if (!task || task.status === "running" || task.status === "cancelling") {
    return;
  }
  backgroundTasks.value = backgroundTasks.value.filter(
    (item) => item.id !== taskId,
  );
}

export function clearFinishedTasks(): void {
  backgroundTasks.value = backgroundTasks.value.filter(
    (task) => task.status === "running" || task.status === "cancelling",
  );
}

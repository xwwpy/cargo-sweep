import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { DeleteResult, ScanProgress, ScanResult } from "./types";
import { demoDelete, demoRoots, demoScan } from "./lib/demo";

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export function isDemoMode(): boolean {
  return !isTauriRuntime() && new URLSearchParams(window.location.search).has("demo");
}

export async function defaultScanRoots(): Promise<string[]> {
  if (isDemoMode()) {
    return demoRoots;
  }
  if (!isTauriRuntime()) {
    return [];
  }
  return invoke<string[]>("default_scan_roots");
}

export async function scanTargets(roots: string[]): Promise<ScanResult> {
  if (isDemoMode()) {
    return demoScan(roots);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<ScanResult>("scan_targets", { roots });
}

export async function deleteTargets(paths: string[]): Promise<DeleteResult> {
  if (isDemoMode()) {
    return demoDelete(paths);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<DeleteResult>("delete_targets", { paths });
}

export async function pickFolders(): Promise<string[]> {
  if (!isTauriRuntime()) {
    return [];
  }

  const selection = await open({
    directory: true,
    multiple: true,
    title: "选择 Rust 项目所在的文件夹",
  });

  if (!selection) {
    return [];
  }
  return Array.isArray(selection) ? selection : [selection];
}

export async function listenToScanProgress(
  handler: (progress: ScanProgress) => void,
): Promise<UnlistenFn> {
  if (!isTauriRuntime()) {
    return () => undefined;
  }
  return listen<ScanProgress>("scan-progress", (event) => handler(event.payload));
}

export function errorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "发生未知错误，请重试。";
}

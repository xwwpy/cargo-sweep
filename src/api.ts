import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type {
  ApplyCargoSourceResult,
  CargoHomeMigrationResult,
  CargoHomeStatus,
  CargoSourceStatus,
  DeleteResult,
  LatestRustVersions,
  RustInstallationProbe,
  RustToolchainOperationResult,
  RustToolchainStatus,
  ScanProgress,
  ScanResult,
} from "./types";
import {
  demoApplyCargoSource,
  demoCargoHomeStatus,
  demoCargoSourceStatus,
  demoCheckRustLatest,
  demoDelete,
  demoInstallRustup,
  demoInstallRustToolchain,
  demoMigrateCargoHome,
  demoPickCargoHomeFolder,
  demoProbeRustInstallation,
  demoRoots,
  demoRustToolchainStatus,
  demoScan,
  demoSetDefaultRustToolchain,
  demoSetRustupPath,
  demoUpdateRustToolchain,
} from "./lib/demo";

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

export async function detectCargoSource(): Promise<CargoSourceStatus> {
  if (isDemoMode()) {
    return demoCargoSourceStatus();
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<CargoSourceStatus>("detect_cargo_source");
}

export async function applyCargoSource(url: string): Promise<ApplyCargoSourceResult> {
  if (isDemoMode()) {
    return demoApplyCargoSource(url);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<ApplyCargoSourceResult>("apply_cargo_source", { url });
}

export async function getCargoHomeStatus(): Promise<CargoHomeStatus> {
  if (isDemoMode()) {
    return demoCargoHomeStatus();
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<CargoHomeStatus>("cargo_home_status");
}

export async function migrateCargoHome(
  target: string,
  createLink: boolean,
): Promise<CargoHomeMigrationResult> {
  if (isDemoMode()) {
    return demoMigrateCargoHome(target, createLink);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<CargoHomeMigrationResult>("migrate_cargo_home", {
    target,
    createLink,
  });
}

export async function pickCargoHomeFolder(): Promise<string | null> {
  if (isDemoMode()) {
    return demoPickCargoHomeFolder();
  }
  if (!isTauriRuntime()) {
    return null;
  }
  const selection = await open({
    directory: true,
    multiple: false,
    title: "选择新的 CARGO_HOME 目录",
  });
  return typeof selection === "string" ? selection : null;
}

export async function probeRustInstallation(): Promise<RustInstallationProbe> {
  if (isDemoMode()) {
    return demoProbeRustInstallation();
  }
  if (!isTauriRuntime()) {
    return { installed: false, rustupInstalled: false, version: null, rustupPath: null };
  }
  return invoke<RustInstallationProbe>("probe_rust_installation");
}

export async function setRustupPath(path: string): Promise<RustToolchainStatus> {
  if (isDemoMode()) {
    return demoSetRustupPath(path);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainStatus>("set_rustup_path", { path });
}

export async function installRustup(): Promise<RustToolchainOperationResult> {
  if (isDemoMode()) {
    return demoInstallRustup();
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainOperationResult>("install_rustup");
}

export async function openPrivacySettings(): Promise<void> {
  if (isDemoMode()) {
    return;
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  await invoke("open_privacy_settings");
}

export async function detectRustToolchain(): Promise<RustToolchainStatus> {
  if (isDemoMode()) {
    return demoRustToolchainStatus();
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainStatus>("detect_rust_toolchain");
}

export async function checkRustToolchainLatest(): Promise<LatestRustVersions> {
  if (isDemoMode()) {
    return demoCheckRustLatest();
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<LatestRustVersions>("check_rust_toolchain_latest");
}

export async function installRustToolchain(
  target: string,
  setDefault = false,
): Promise<RustToolchainOperationResult> {
  if (isDemoMode()) {
    return demoInstallRustToolchain(target, setDefault);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainOperationResult>("install_rust_toolchain", {
    target,
    setDefault,
  });
}

export async function setDefaultRustToolchain(
  target: string,
): Promise<RustToolchainOperationResult> {
  if (isDemoMode()) {
    return demoSetDefaultRustToolchain(target);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainOperationResult>("set_default_rust_toolchain", {
    target,
  });
}

export async function updateRustToolchain(
  target: string | null = null,
): Promise<RustToolchainOperationResult> {
  if (isDemoMode()) {
    return demoUpdateRustToolchain(target);
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  return invoke<RustToolchainOperationResult>("update_rust_toolchain", {
    target,
  });
}

export async function openFolder(path: string): Promise<void> {
  if (isDemoMode()) {
    throw new Error("演示模式无法打开本地目录");
  }
  if (!isTauriRuntime()) {
    throw new Error("请通过 Tauri 桌面应用运行 Cargo Sweep");
  }
  await invoke("open_folder", { path });
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

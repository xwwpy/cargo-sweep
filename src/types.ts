export interface TargetInfo {
  path: string;
  projectName: string;
  projectPath: string;
  sizeBytes: number;
  fileCount: number;
  modifiedMs: number | null;
}

export interface ScanResult {
  targets: TargetInfo[];
  scannedRoots: number;
  requestedRoots: number;
  skippedRoots: string[];
  scannedDirs: number;
  elapsedMs: number;
}

export interface ScanProgress {
  stage: "discovering" | "measuring" | "done" | string;
  root: string | null;
  rootsScanned: number;
  rootsTotal: number;
  found: number;
  measured: number;
}

export interface DeleteFailure {
  path: string;
  message: string;
}

export interface DeleteResult {
  deleted: string[];
  freedBytes: number;
  failures: DeleteFailure[];
  elapsedMs: number;
}

export interface NamedCargoSource {
  name: string;
  index: string;
  kind: string;
}

export interface CargoSourceStatus {
  cargoHome: string;
  configPath: string;
  configDir: string;
  configExists: boolean;
  cargoVersion: string | null;
  currentName: string;
  currentIndex: string;
  currentIsOfficial: boolean;
  replaceWith: string | null;
  configuredSources: NamedCargoSource[];
  warning: string | null;
}

export interface ApplyCargoSourceResult {
  backupPath: string | null;
  status: CargoSourceStatus;
}

export interface InstalledRustToolchain {
  name: string;
  active: boolean;
}

export interface RustToolchainStatus {
  version: string;
  rustcVersion: string;
  release: string;
  host: string | null;
  channel: string | null;
  activeToolchain: string | null;
  toolchains: InstalledRustToolchain[];
  cargoVersion: string | null;
  rustupVersion: string | null;
}

export interface RustToolchainUpdateResult {
  success: boolean;
  output: string;
  status: RustToolchainStatus;
}

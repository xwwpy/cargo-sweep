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
  channel: string;
  release: string | null;
  rustcVersion: string | null;
  active: boolean;
  default: boolean;
}

export interface RustChannel {
  channel: string;
  installed: boolean;
  toolchain: string | null;
  release: string | null;
  rustcVersion: string | null;
  active: boolean;
  default: boolean;
}

export interface RustToolchainStatus {
  installed: boolean;
  rustupInstalled: boolean;
  version: string | null;
  rustcVersion: string | null;
  host: string | null;
  activeToolchain: string | null;
  defaultToolchain: string | null;
  toolchains: InstalledRustToolchain[];
  channels: RustChannel[];
  cargoVersion: string | null;
  rustupVersion: string | null;
  rustupPath: string | null;
  rustupCustomPath: string | null;
}

export interface LatestRustToolchain {
  channel: string;
  version: string;
  release: string;
  date: string | null;
}

export interface LatestRustVersions {
  stable: LatestRustToolchain | null;
  nightly: LatestRustToolchain | null;
  historicalVersions: string[];
  error: string | null;
}

export interface RustToolchainOperationResult {
  success: boolean;
  output: string;
  status: RustToolchainStatus;
}

export interface RustInstallationProbe {
  installed: boolean;
  rustupInstalled: boolean;
  version: string | null;
  rustupPath: string | null;
}

export interface CargoHomeStatus {
  current: string;
  default: string;
  source: string;
  cargoBin: string;
  rustupHome: string;
  pathContainsCargoBin: boolean;
  linkExists: boolean;
  linkTarget: string | null;
  linkMatches: boolean;
  shellProfile: string | null;
}

export interface CargoHomeMigrationResult {
  output: string;
  status: CargoHomeStatus;
}

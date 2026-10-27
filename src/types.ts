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

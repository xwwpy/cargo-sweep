import type {
  ApplyCargoSourceResult,
  CargoSourceStatus,
  DeleteResult,
  ScanResult,
  TargetInfo,
} from "../types";

const HOUR = 60 * 60 * 1000;

function target(
  projectName: string,
  projectPath: string,
  sizeBytes: number,
  fileCount: number,
  modifiedHoursAgo: number,
): TargetInfo {
  return {
    path: `${projectPath}\\target`,
    projectName,
    projectPath,
    sizeBytes,
    fileCount,
    modifiedMs: Date.now() - modifiedHoursAgo * HOUR,
  };
}

const demoTargets: TargetInfo[] = [
  target("aurora-engine", "C:\\dev\\aurora-engine", 5_170_000_000, 48_219, 2),
  target("telemetry-gateway", "C:\\dev\\telemetry-gateway", 2_260_000_000, 21_804, 6),
  target("pixel-forge", "D:\\projects\\pixel-forge", 1_390_000_000, 17_553, 27),
  target("cargo-sweep", "C:\\dev\\cargo-sweep", 686_000_000, 8_042, 1),
  target("cli-toolkit", "D:\\projects\\cli-toolkit", 372_000_000, 3_918, 74),
];

export const demoRoots = ["C:\\dev", "D:\\projects"];

export async function demoScan(_roots: string[]): Promise<ScanResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 900));
  return {
    targets: demoTargets.map((item) => ({ ...item })),
    scannedRoots: 2,
    requestedRoots: 2,
    skippedRoots: [],
    scannedDirs: 1_284,
    elapsedMs: 2_841,
  };
}

const OFFICIAL_INDEX = "sparse+https://index.crates.io/";

const demoSourceStatus: CargoSourceStatus = {
  cargoHome: "C:\\Users\\PC\\.cargo",
  configPath: "C:\\Users\\PC\\.cargo\\config.toml",
  configDir: "C:\\Users\\PC\\.cargo",
  configExists: true,
  cargoVersion: "cargo 1.99.1",
  currentName: "rsproxy-sparse",
  currentIndex: "sparse+https://rsproxy.cn/index/",
  currentIsOfficial: false,
  replaceWith: "rsproxy-sparse",
  configuredSources: [
    {
      name: "rsproxy-sparse",
      index: "sparse+https://rsproxy.cn/index/",
      kind: "registry",
    },
  ],
  warning: null,
};

export async function demoCargoSourceStatus(): Promise<CargoSourceStatus> {
  await new Promise((resolve) => window.setTimeout(resolve, 420));
  return {
    ...demoSourceStatus,
    configuredSources: demoSourceStatus.configuredSources.map((source) => ({ ...source })),
  };
}

export async function demoApplyCargoSource(url: string): Promise<ApplyCargoSourceResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 720));
  const status: CargoSourceStatus = {
    ...demoSourceStatus,
    currentName: url === OFFICIAL_INDEX ? "crates.io" : "cargo-sweep-managed",
    currentIndex: url,
    currentIsOfficial: url === OFFICIAL_INDEX,
    replaceWith: url === OFFICIAL_INDEX ? null : "cargo-sweep-managed",
    configuredSources: [
      {
        name: "cargo-sweep-managed",
        index: url,
        kind: "registry",
      },
    ],
  };
  return {
    backupPath:
      url === OFFICIAL_INDEX ? null : "C:\\Users\\PC\\.cargo\\config.toml.cargo-sweep-backup-demo",
    status,
  };
}

export async function demoDelete(paths: string[]): Promise<DeleteResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 700));
  const freedBytes = demoTargets
    .filter((item) => paths.includes(item.path))
    .reduce((total, item) => total + item.sizeBytes, 0);

  return {
    deleted: paths,
    freedBytes,
    failures: [],
    elapsedMs: 612,
  };
}

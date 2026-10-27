import type {
  ApplyCargoSourceResult,
  CargoSourceStatus,
  DeleteResult,
  RustToolchainStatus,
  RustToolchainUpdateResult,
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
  target("wasm-studio", "C:\\dev\\wasm-studio", 214_000_000, 2_840, 13),
  target("edge-proxy", "D:\\projects\\edge-proxy", 96_000_000, 1_204, 96),
  target("sandbox-lab", "D:\\projects\\sandbox-lab", 48_000_000, 612, 240),
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
  cargoVersion: "1.99.1",
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

const demoToolchains = [
  { name: "stable-x86_64-pc-windows-msvc", active: true },
  { name: "nightly-x86_64-pc-windows-msvc", active: false },
  { name: "1.98.0-x86_64-pc-windows-msvc", active: false },
];

function demoToolchainStatus(version: string): RustToolchainStatus {
  return {
    version,
    rustcVersion: `rustc ${version} (demo build)`,
    release: version,
    host: "x86_64-pc-windows-msvc",
    channel: "stable",
    activeToolchain: "stable-x86_64-pc-windows-msvc",
    toolchains: demoToolchains.map((toolchain) => ({ ...toolchain })),
    cargoVersion: "1.99.1",
    rustupVersion: "1.29.2",
  };
}

export async function demoRustToolchainStatus(): Promise<RustToolchainStatus> {
  await new Promise((resolve) => window.setTimeout(resolve, 420));
  return demoToolchainStatus("1.99.1");
}

export async function demoUpdateRustToolchain(): Promise<RustToolchainUpdateResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 1800));
  return {
    success: true,
    output: [
      "info: syncing channel updates for 'stable-x86_64-pc-windows-msvc'",
      "info: latest update on 2026-10-27, rust version 1.101.0",
      "info: downloading component 'rustc'",
      "info: downloading component 'cargo'",
      "info: installing component 'rustc'",
      "info: installing component 'cargo'",
      "info: default toolchain set to 'stable-x86_64-pc-windows-msvc'",
      "",
      "  stable-x86_64-pc-windows-msvc updated - rustc 1.101.0",
    ].join("\n"),
    status: demoToolchainStatus("1.101.0"),
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

import type {
  ApplyCargoSourceResult,
  CargoSourceStatus,
  DeleteResult,
  LatestRustVersions,
  RustInstallationProbe,
  RustToolchainOperationResult,
  RustToolchainStatus,
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

const DEMO_HOST = "x86_64-pc-windows-msvc";
const demoNoRust = new URLSearchParams(window.location.search).has("norust");
let demoInstalled = !demoNoRust;
let demoStable = "1.99.1";
let demoNightly = "1.101.0";
let demoDefault = "stable";
const demoPinned = new Map<string, string>([["1.98.0", "1.98.0"]]);

function demoToolchains() {
  if (!demoInstalled) {
    return [];
  }
  return [
    {
      name: `stable-${DEMO_HOST}`,
      channel: "stable",
      release: demoStable,
      rustcVersion: `rustc ${demoStable} (demo build)`,
      active: demoDefault === "stable",
      default: demoDefault === "stable",
    },
    {
      name: `nightly-${DEMO_HOST}`,
      channel: "nightly",
      release: demoNightly,
      rustcVersion: `rustc ${demoNightly}-nightly (demo build)`,
      active: demoDefault === "nightly",
      default: demoDefault === "nightly",
    },
    ...[...demoPinned.entries()].map(([version]) => ({
      name: `${version}-${DEMO_HOST}`,
      channel: "pinned",
      release: version,
      rustcVersion: `rustc ${version} (demo build)`,
      active: demoDefault === version,
      default: demoDefault === version,
    })),
  ];
}

function demoToolchainStatus(): RustToolchainStatus {
  const toolchains = demoToolchains();
  const channels = ["stable", "nightly"].map((channel) => {
    const installed = toolchains.find((toolchain) => toolchain.channel === channel);
    return {
      channel,
      installed: Boolean(installed),
      toolchain: installed?.name ?? null,
      release: installed?.release ?? null,
      rustcVersion: installed?.rustcVersion ?? null,
      active: installed?.active ?? false,
      default: installed?.default ?? false,
    };
  });
  const active = toolchains.find((toolchain) => toolchain.active);
  return {
    installed: demoInstalled,
    rustupInstalled: true,
    version: active?.release ?? null,
    rustcVersion: active?.rustcVersion ?? null,
    host: DEMO_HOST,
    activeToolchain: active?.name ?? null,
    defaultToolchain: demoDefault ? `${demoDefault}-${DEMO_HOST}` : null,
    toolchains,
    channels,
    cargoVersion: "1.99.1",
    rustupVersion: "1.29.2",
  };
}

export async function demoProbeRustInstallation(): Promise<RustInstallationProbe> {
  return {
    installed: demoInstalled,
    rustupInstalled: true,
    version: demoInstalled ? `rustc ${demoStable} (demo build)` : null,
  };
}

export async function demoRustToolchainStatus(): Promise<RustToolchainStatus> {
  await new Promise((resolve) => window.setTimeout(resolve, 420));
  return demoToolchainStatus();
}

export async function demoCheckRustLatest(): Promise<LatestRustVersions> {
  await new Promise((resolve) => window.setTimeout(resolve, 520));
  return {
    stable: {
      channel: "stable",
      version: "1.101.0 (demo build)",
      release: "1.101.0",
      date: "2026-10-27",
    },
    nightly: {
      channel: "nightly",
      version: "1.103.0-nightly (demo build)",
      release: "1.103.0-nightly",
      date: "2026-10-27",
    },
    historicalVersions: ["1.100.0", "1.99.0", "1.98.0", "1.97.0", "1.96.0", "1.95.0"],
    error: null,
  };
}

function demoToolchainName(target: string): string {
  if (target === "stable" || target === "nightly") {
    return `${target}-${DEMO_HOST}`;
  }
  return `${target}-${DEMO_HOST}`;
}

function demoOperation(output: string): RustToolchainOperationResult {
  return {
    success: true,
    output,
    status: demoToolchainStatus(),
  };
}

export async function demoInstallRustToolchain(
  target: string,
  setDefault: boolean,
): Promise<RustToolchainOperationResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 1400));
  if (target === "stable") {
    demoStable = "1.101.0";
    demoDefault = setDefault || !demoInstalled ? "stable" : demoDefault;
    demoInstalled = true;
  } else if (target === "nightly") {
    demoNightly = "1.103.0";
    demoDefault = setDefault || !demoInstalled ? "nightly" : demoDefault;
    demoInstalled = true;
  } else {
    demoPinned.set(target, target);
    demoDefault = setDefault || !demoInstalled ? target : demoDefault;
    demoInstalled = true;
  }
  const name = demoToolchainName(target);
  return demoOperation(
    [
      `info: syncing component updates for '${name}'`,
      `info: installing component 'rustc'`,
      `info: installing component 'cargo'`,
      `info: default toolchain set to '${demoDefault}-${DEMO_HOST}'`,
    ].join("\n"),
  );
}

export async function demoSetDefaultRustToolchain(
  target: string,
): Promise<RustToolchainOperationResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 700));
  const channel = target.split("-")[0];
  demoDefault = channel;
  return demoOperation(
    `info: default toolchain set to '${demoToolchainName(channel)}'`,
  );
}

export async function demoUpdateRustToolchain(
  target: string | null,
): Promise<RustToolchainOperationResult> {
  await new Promise((resolve) => window.setTimeout(resolve, 1600));
  const channel = (target ?? demoDefault).split("-")[0];
  if (channel === "nightly") {
    demoNightly = "1.103.0";
  } else {
    demoStable = "1.101.0";
  }
  return demoOperation(
    [
      `info: syncing channel updates for '${channel}-${DEMO_HOST}'`,
      `info: downloading component 'rustc'`,
      `info: installing component 'rustc'`,
      `  ${channel}-${DEMO_HOST} updated`,
    ].join("\n"),
  );
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

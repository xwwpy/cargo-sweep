<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { ElMessage, ElMessageBox } from "element-plus";
import {
  Check,
  Connection,
  Delete,
  FolderOpened,
  InfoFilled,
  Link,
  Plus,
  Refresh,
  Timer,
  WarningFilled,
} from "@element-plus/icons-vue";
import {
  applyCargoSource,
  detectCargoSource,
  errorMessage,
  openFolder,
  testSourceSpeed,
} from "../api";
import { cleanPath, formatBytes } from "../lib/format";
import type { CargoSourceStatus, SourceSpeedResult } from "../types";

const props = withDefaults(defineProps<{ embedded?: boolean }>(), {
  embedded: false,
});

interface SourceOption {
  id: string;
  name: string;
  provider: string;
  url: string;
  description: string;
  badge: string;
  custom?: boolean;
}

const STORAGE_KEY = "cargo-sweep.custom-sources.v1";
const OFFICIAL_INDEX = "sparse+https://index.crates.io/";

const builtinSources: SourceOption[] = [
  {
    id: "official",
    name: "crates.io 官方",
    provider: "Rust 官方",
    url: OFFICIAL_INDEX,
    description: "稳定、无第三方依赖，国内网络环境可能较慢。",
    badge: "官方",
  },
  {
    id: "rsproxy",
    name: "RsProxy",
    provider: "字节跳动",
    url: "sparse+https://rsproxy.cn/index/",
    description: "国内访问速度快，提供 crates 与 rustup 镜像。",
    badge: "推荐",
  },
  {
    id: "ustc",
    name: "中科大 USTC",
    provider: "中国科学技术大学",
    url: "sparse+https://mirrors.ustc.edu.cn/crates.io-index/",
    description: "教育网访问稳定，支持 sparse 索引。",
    badge: "镜像",
  },
  {
    id: "tuna",
    name: "清华 TUNA",
    provider: "清华大学",
    url: "sparse+https://mirrors.tuna.tsinghua.edu.cn/crates.io-index/",
    description: "清华开源镜像站，国内访问速度优秀。",
    badge: "镜像",
  },
  {
    id: "aliyun",
    name: "阿里云",
    provider: "阿里云",
    url: "sparse+https://mirrors.aliyun.com/crates.io-index/",
    description: "阿里云开源镜像，支持稀疏索引协议。",
    badge: "镜像",
  },
  {
    id: "sjtug",
    name: "上海交大 SJTUG",
    provider: "上海交通大学",
    url: "sparse+https://mirrors.sjtug.sjtu.edu.cn/crates.io-index/",
    description: "SJTUG 镜像，提供 crates.io 索引同步。",
    badge: "镜像",
  },
];

const status = ref<CargoSourceStatus | null>(null);
const loading = ref(true);
const applying = ref(false);
const error = ref<string | null>(null);
const selectedId = ref("");
const customSources = ref<SourceOption[]>([]);
const customName = ref("");
const customUrl = ref("");
const lastBackup = ref<string | null>(null);
const speedStates = ref<
  Record<string, { testing: boolean; result: SourceSpeedResult | null }>
>({});
const testingAll = ref(false);

const allSources = computed(() => [...builtinSources, ...customSources.value]);
const selectedSource = computed(
  () => allSources.value.find((source) => source.id === selectedId.value) ?? null,
);
const currentSource = computed(() => {
  const current = status.value;
  if (!current) {
    return null;
  }
  return (
    allSources.value.find((source) => sameUrl(source.url, current.currentIndex)) ?? null
  );
});
const currentName = computed(() => {
  const current = status.value;
  if (!current) {
    return "检测中…";
  }
  if (currentSource.value) {
    return currentSource.value.name;
  }
  if (current.currentIsOfficial) {
    return "crates.io 官方";
  }
  return current.currentName || "自定义源";
});
const canApply = computed(
  () =>
    selectedSource.value !== null &&
    !applying.value &&
    status.value !== null &&
    !sameUrl(selectedSource.value.url, status.value.currentIndex),
);

function sameUrl(left: string, right: string): boolean {
  return (
    left.trim().replace(/\/+$/, "").toLowerCase() ===
    right.trim().replace(/\/+$/, "").toLowerCase()
  );
}

function isCurrent(source: SourceOption): boolean {
  return status.value !== null && sameUrl(source.url, status.value.currentIndex);
}

function cargoSpeedUrl(source: SourceOption): string {
  const raw = source.url.trim().replace(/^git\+/, "");
  const sparse = raw.replace(/^sparse\+/, "");
  const base = sparse.endsWith("/") ? sparse : `${sparse}/`;
  return raw.startsWith("sparse+") ? `${base}se/rd/serde` : `${base}config.json`;
}

function speedLabel(sourceId: string): string {
  const state = speedStates.value[sourceId];
  if (!state) {
    return "";
  }
  if (state.testing) {
    return "测速中…";
  }
  if (!state.result) {
    return "";
  }
  if (!state.result.ok) {
    return "测速失败";
  }
  return `${state.result.latencyMs}ms · ${formatBytes(state.result.speedBps)}/s`;
}

async function testCargoSource(source: SourceOption): Promise<void> {
  if (speedStates.value[source.id]?.testing) {
    return;
  }
  speedStates.value[source.id] = { testing: true, result: null };
  try {
    const result = await testSourceSpeed(cargoSpeedUrl(source), 1024 * 1024);
    speedStates.value[source.id] = { testing: false, result };
  } catch (reason) {
    speedStates.value[source.id] = {
      testing: false,
      result: {
        url: source.url,
        ok: false,
        status: null,
        latencyMs: 0,
        durationMs: 0,
        bytes: 0,
        speedBps: 0,
        error: errorMessage(reason),
      },
    };
  }
}

async function testAllSources(): Promise<void> {
  if (testingAll.value) {
    return;
  }
  testingAll.value = true;
  try {
    await Promise.all(allSources.value.map((source) => testCargoSource(source)));
  } finally {
    testingAll.value = false;
  }
}

function loadCustomSources() {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return;
    }
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) {
      return;
    }
    customSources.value = parsed
      .filter(
        (item): item is SourceOption =>
          typeof item === "object" &&
          item !== null &&
          typeof (item as SourceOption).id === "string" &&
          typeof (item as SourceOption).name === "string" &&
          typeof (item as SourceOption).url === "string",
      )
      .map((item) => ({ ...item, custom: true }));
  } catch {
    localStorage.removeItem(STORAGE_KEY);
  }
}

function saveCustomSources() {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(customSources.value));
  } catch {
    // Custom source persistence is optional.
  }
}

function selectSource(source: SourceOption) {
  selectedId.value = source.id;
}

async function refresh() {
  loading.value = true;
  error.value = null;
  try {
    status.value = await detectCargoSource();
    const matched = currentSource.value;
    selectedId.value = matched?.id ?? "";
    lastBackup.value = null;
  } catch (reason) {
    error.value = errorMessage(reason);
  } finally {
    loading.value = false;
  }
}

async function applySelected() {
  const source = selectedSource.value;
  if (!source || !status.value) {
    return;
  }
  if (isCurrent(source)) {
    ElMessage.info("当前已经使用这个下载源");
    return;
  }

  const confirmed = await ElMessageBox.confirm(
    `将全局 Cargo 下载源切换为「${source.name}」。切换前会备份 ${cleanPath(status.value.configPath)}。`,
    "切换 Cargo 下载源",
    {
      confirmButtonText: "切换并备份",
      cancelButtonText: "取消",
      type: "warning",
    },
  )
    .then(() => true)
    .catch(() => false);

  if (!confirmed) {
    return;
  }

  applying.value = true;
  try {
    const result = await applyCargoSource(source.url);
    status.value = result.status;
    selectedId.value = source.id;
    lastBackup.value = result.backupPath;
    ElMessage.success(`已切换到 ${source.name}`);
  } catch (reason) {
    ElMessage.error(`切换失败：${errorMessage(reason)}`);
  } finally {
    applying.value = false;
  }
}

function addCustomSource() {
  const name = customName.value.trim();
  const url = customUrl.value.trim();
  if (!name) {
    ElMessage.warning("请填写源名称");
    return;
  }
  if (!/^(sparse\+https:\/\/|https:\/\/)/.test(url) || /\s/.test(url)) {
    ElMessage.warning("索引地址需以 sparse+https:// 或 https:// 开头，且不能包含空格");
    return;
  }
  if (allSources.value.some((source) => sameUrl(source.url, url))) {
    ElMessage.info("这个索引地址已经存在");
    return;
  }

  const source: SourceOption = {
    id: `custom-${Date.now()}`,
    name,
    provider: "自定义",
    url,
    description: "用户自定义 Cargo 索引。",
    badge: "自定义",
    custom: true,
  };
  customSources.value = [...customSources.value, source];
  saveCustomSources();
  selectedId.value = source.id;
  customName.value = "";
  customUrl.value = "";
  ElMessage.success("已添加自定义源，点击下方按钮应用");
}

function removeCustomSource(source: SourceOption) {
  customSources.value = customSources.value.filter((item) => item.id !== source.id);
  saveCustomSources();
  if (selectedId.value === source.id) {
    selectedId.value = currentSource.value?.id ?? "";
  }
}

async function openConfigDir() {
  if (!status.value?.configDir) {
    return;
  }
  try {
    await openFolder(cleanPath(status.value.configDir));
  } catch (reason) {
    ElMessage.error(`打开配置目录失败：${errorMessage(reason)}`);
  }
}

onMounted(() => {
  loadCustomSources();
  void refresh();
});
</script>

<template>
  <section class="source-page" :class="{ 'is-embedded': props.embedded }">
    <header v-if="!props.embedded" class="source-hero">
      <div>
        <span class="eyebrow"><i /> CARGO REGISTRY</span>
        <h1>下载源，<span>选一个更快的。</span></h1>
        <p>读取全局 Cargo 配置，查看或切换 crates.io 下载源。</p>
      </div>
      <el-button class="refresh-button" :loading="loading" @click="refresh">
        <el-icon v-if="!loading"><Refresh /></el-icon>
        重新检测
      </el-button>
    </header>

    <div v-else class="embedded-toolbar">
      <div>
        <span class="section-kicker">CARGO REGISTRY</span>
        <h2>Cargo 下载源</h2>
      </div>
      <el-button class="refresh-button" :loading="loading" @click="refresh">
        <el-icon v-if="!loading"><Refresh /></el-icon>
        重新检测
      </el-button>
    </div>

    <div v-if="error" class="source-error">
      <div class="error-icon"><el-icon><WarningFilled /></el-icon></div>
      <div>
        <h3>无法读取 Cargo 配置</h3>
        <p>{{ error }}</p>
      </div>
      <el-button class="retry-button" @click="refresh">重试</el-button>
    </div>

    <div v-else-if="loading && !status" class="source-loading">
      <div class="loading-orbit">
        <span class="loading-ring ring-one" />
        <span class="loading-ring ring-two" />
        <span class="loading-core" />
      </div>
      <h3>正在检测 Cargo 下载源</h3>
      <p>读取全局配置文件与环境信息</p>
    </div>

    <template v-else-if="status">
      <section class="status-panel">
        <div class="status-mark"><el-icon><Connection /></el-icon></div>
        <div class="status-copy">
          <span class="status-label">当前下载源</span>
          <h2>{{ currentName }}</h2>
          <code>{{ status.currentIndex || "未找到有效的索引地址" }}</code>
        </div>
        <div class="status-details">
          <div class="status-row">
            <span>配置文件</span>
            <button class="path-button" type="button" @click="openConfigDir">
              {{ cleanPath(status.configPath) }}
              <el-icon><FolderOpened /></el-icon>
            </button>
          </div>
          <div class="status-row">
            <span>Cargo</span>
            <strong>{{ status.cargoVersion || "未检测到 cargo 命令" }}</strong>
          </div>
        </div>
        <span v-if="status.currentIsOfficial" class="official-pill">OFFICIAL</span>
      </section>

      <div v-if="status.warning" class="config-warning">
        <el-icon><WarningFilled /></el-icon>
        <span>{{ status.warning }}</span>
      </div>

      <div v-if="lastBackup" class="backup-note">
        <el-icon><Check /></el-icon>
        <span>原配置已备份至：<code>{{ cleanPath(lastBackup) }}</code></span>
      </div>

      <div class="source-heading">
        <div>
          <span class="section-kicker">MIRRORS</span>
          <h2>推荐镜像</h2>
        </div>
        <button
          type="button"
          class="speed-all-button"
          :disabled="testingAll"
          @click="testAllSources"
        >
          <el-icon :class="{ spinning: testingAll }"><Timer /></el-icon>
          {{ testingAll ? "全部测速中…" : "全部测速" }}
        </button>
      </div>

      <div class="source-grid">
        <div
          v-for="source in allSources"
          :key="source.id"
          class="source-card"
          :class="{
            selected: selectedId === source.id,
            current: isCurrent(source),
            official: source.id === 'official',
          }"
          role="button"
          tabindex="0"
          @click="selectSource(source)"
          @keydown.enter.prevent="selectSource(source)"
          @keydown.space.prevent="selectSource(source)"
        >
          <div class="card-top">
            <span class="source-name">{{ source.name }}</span>
            <span class="source-badge" :class="{ custom: source.custom }">
              {{ source.badge }}
            </span>
          </div>
          <div class="source-provider">{{ source.provider }}</div>
          <p>{{ source.description }}</p>
          <code>{{ source.url }}</code>
          <div class="source-speed-line" @click.stop>
            <button
              type="button"
              class="source-speed-button"
              :disabled="speedStates[source.id]?.testing"
              @click.stop="testCargoSource(source)"
            >
              <el-icon
                :class="{ spinning: speedStates[source.id]?.testing }"
              >
                <Timer />
              </el-icon>
              {{ speedStates[source.id]?.testing ? "测速中" : "测速" }}
            </button>
            <span
              v-if="speedLabel(source.id)"
              class="source-speed-result"
              :class="{
                failed: speedStates[source.id]?.result?.error,
              }"
              :title="speedStates[source.id]?.result?.error ?? ''"
            >
              {{ speedLabel(source.id) }}
            </span>
          </div>
          <div class="card-footer">
            <span v-if="isCurrent(source)" class="current-label">
              <el-icon><Check /></el-icon>
              当前使用
            </span>
            <span v-else class="select-label">
              {{ selectedId === source.id ? "已选择" : "选择此源" }}
            </span>
            <button
              v-if="source.custom"
              class="remove-custom"
              type="button"
              aria-label="删除自定义源"
              @click.stop="removeCustomSource(source)"
            >
              <el-icon><Delete /></el-icon>
            </button>
          </div>
        </div>

        <div class="source-card add-card">
          <div class="add-card-head">
            <span class="add-icon"><el-icon><Link /></el-icon></span>
            <div>
              <span class="source-name">自定义源</span>
              <span class="source-provider">添加自己的 Cargo 索引</span>
            </div>
          </div>
          <el-input v-model="customName" placeholder="源名称，例如 公司内网镜像" />
          <el-input
            v-model="customUrl"
            placeholder="sparse+https://mirror.example.com/index/"
          />
          <el-button class="add-source-button" @click="addCustomSource">
            <el-icon><Plus /></el-icon>
            保存自定义源
          </el-button>
        </div>
      </div>

      <section
        v-if="selectedSource && !isCurrent(selectedSource)"
        class="apply-panel"
      >
        <div class="apply-copy">
          <span class="apply-label">准备切换</span>
          <strong>{{ selectedSource?.name ?? "尚未选择下载源" }}</strong>
          <p v-if="selectedSource">
            {{ selectedSource.url }}
          </p>
          <p v-else>在上方选择一个镜像，或添加自定义 Cargo 索引。</p>
        </div>
        <div class="apply-actions">
          <span class="backup-hint">
            <el-icon><InfoFilled /></el-icon>
            自动备份原配置（仅保留上一次 .old）
          </span>
          <el-button
            type="primary"
            class="apply-button"
            :disabled="!canApply"
            :loading="applying"
            @click="applySelected"
          >
            <el-icon v-if="!applying"><Check /></el-icon>
            {{ selectedSource && isCurrent(selectedSource) ? "当前已使用" : "切换到此源" }}
          </el-button>
        </div>
      </section>

      <div class="config-note">
        <el-icon><InfoFilled /></el-icon>
        <p>
          仅修改全局 <code>{{ cleanPath(status?.configPath ?? "") }}</code>；项目内的
          <code>.cargo/config.toml</code> 优先级更高。
        </p>
      </div>

      <section v-if="(status?.configuredSources?.length ?? 0) > 0" class="detected-section">
        <span class="section-kicker">DETECTED</span>
        <h2>配置中已存在的源</h2>
        <div class="detected-list">
          <div
            v-for="source in status?.configuredSources ?? []"
            :key="source.name + source.index"
          >
            <span>{{ source.name }}</span>
            <code>{{ source.index }}</code>
            <em>{{ source.kind }}</em>
          </div>
        </div>
      </section>
    </template>
  </section>
</template>

<style scoped>
.source-page {
  padding: 25px 2px 112px;
  animation: page-in 480ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.source-page.is-embedded {
  padding-top: 2px;
  padding-bottom: 20px;
}

.embedded-toolbar {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding: 2px 2px 12px;
}

.embedded-toolbar .section-kicker {
  display: block;
  margin-bottom: 2px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 650;
  letter-spacing: 1.7px;
  color: #a78bfa;
}

.embedded-toolbar h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 15px;
  font-weight: 720;
  color: var(--app-ink, #241a35);
}

.source-page.is-embedded .apply-panel {
  position: sticky;
  right: auto;
  bottom: 10px;
  left: auto;
  margin-top: 16px;
}

.source-hero {
  display: flex;
  gap: 20px;
  align-items: flex-end;
  justify-content: space-between;
  padding-bottom: 22px;
}

.source-hero > div {
  max-width: 720px;
}

.eyebrow {
  display: flex;
  gap: 7px;
  align-items: center;
  margin-bottom: 10px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 650;
  letter-spacing: 2.1px;
  color: #0f766e;
}

.eyebrow i {
  width: 16px;
  height: 1px;
  background: #14b8a6;
}

.source-hero h1 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: clamp(30px, 3.1vw, 38px);
  font-weight: 650;
  line-height: 1.2;
  color: #101828;
  letter-spacing: -1.4px;
}

.source-hero h1 span {
  color: #0f766e;
}

.source-hero p {
  max-width: 650px;
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.8;
  color: #667085;
}

.refresh-button {
  flex: 0 0 auto;
  height: 32px;
  padding: 0 11px;
  font-size: 10.5px;
  border-radius: 8px;
}

.source-error {
  display: flex;
  gap: 14px;
  align-items: flex-start;
  padding: 18px;
  background: #fff;
  border: 1px solid #fecdca;
  border-radius: 15px;
}

.error-icon {
  display: grid;
  flex: 0 0 36px;
  width: 36px;
  height: 36px;
  font-size: 18px;
  color: #d92d20;
  background: #fef3f2;
  border-radius: 10px;
  place-items: center;
}

.source-error h3 {
  margin: 2px 0 4px;
  font-size: 13px;
  color: #101828;
}

.source-error p {
  margin: 0;
  font-size: 10.5px;
  line-height: 1.6;
  color: #667085;
}

.retry-button {
  margin-left: auto;
}

.source-loading {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  min-height: 430px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 17px;
}

.loading-orbit {
  position: relative;
  display: grid;
  width: 74px;
  height: 74px;
  margin-bottom: 18px;
  place-items: center;
}

.loading-ring {
  position: absolute;
  border: 1px solid #b9e9e0;
  border-radius: 50%;
}

.ring-one {
  width: 72px;
  height: 72px;
  border-style: dashed;
  animation: spin 8s linear infinite;
}

.ring-two {
  width: 46px;
  height: 46px;
  animation: spin-reverse 5s linear infinite;
}

.loading-core {
  width: 8px;
  height: 8px;
  background: #0f766e;
  border-radius: 50%;
  box-shadow: 0 0 0 5px rgba(15, 118, 110, 0.1);
}

.source-loading h3 {
  margin: 0;
  font-size: 13px;
  color: #344054;
}

.source-loading p {
  margin: 6px 0 0;
  font-size: 10.5px;
  color: #98a2b3;
}

.status-panel {
  position: relative;
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 20px 22px;
  overflow: hidden;
  color: #f0fffb;
  background:
    radial-gradient(circle at 85% 0%, rgba(94, 234, 212, 0.16), transparent 32%),
    linear-gradient(135deg, #0f172a, #111d2f 58%, #0b3b38);
  border: 1px solid rgba(15, 118, 110, 0.22);
  border-radius: 17px;
  box-shadow: 0 18px 42px rgba(16, 24, 40, 0.12);
}

.status-panel::after {
  position: absolute;
  top: -90px;
  right: 190px;
  width: 210px;
  height: 210px;
  content: "";
  border: 1px solid rgba(148, 233, 217, 0.09);
  border-radius: 50%;
  pointer-events: none;
}

.status-mark {
  display: grid;
  flex: 0 0 44px;
  width: 44px;
  height: 44px;
  font-size: 20px;
  color: #0b3b38;
  background: #5eead4;
  border-radius: 12px;
  box-shadow: 0 8px 20px rgba(45, 212, 191, 0.24);
  place-items: center;
}

.status-copy {
  flex: 1;
  min-width: 0;
}

.status-label {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 600;
  letter-spacing: 1.4px;
  color: #8fb6b0;
  text-transform: uppercase;
}

.status-copy h2 {
  margin: 3px 0 4px;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 20px;
  font-weight: 650;
  letter-spacing: -0.3px;
  color: #f0fffb;
}

.status-copy code {
  display: block;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9.5px;
  color: #94c9c0;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.status-details {
  display: flex;
  flex: 0 0 auto;
  flex-direction: column;
  gap: 7px;
  max-width: 350px;
  padding-left: 20px;
  border-left: 1px solid rgba(255, 255, 255, 0.09);
}

.status-row {
  display: flex;
  gap: 8px;
  align-items: center;
  justify-content: flex-end;
  font-size: 9px;
  color: #789489;
}

.status-row strong {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9.5px;
  font-weight: 500;
  color: #c5ded9;
}

.path-button {
  display: flex;
  gap: 5px;
  align-items: center;
  max-width: 250px;
  padding: 0;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9px;
  color: #c5ded9;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
  cursor: pointer;
  background: transparent;
  border: 0;
}

.path-button:hover {
  color: #5eead4;
}

.path-button .el-icon {
  flex: 0 0 auto;
}

.official-pill {
  position: absolute;
  top: 14px;
  right: 17px;
  padding: 3px 6px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 7.5px;
  font-weight: 700;
  letter-spacing: 1px;
  color: #9af5e5;
  border: 1px solid rgba(94, 234, 212, 0.2);
  border-radius: 5px;
}

.config-warning,
.backup-note {
  display: flex;
  gap: 7px;
  align-items: flex-start;
  padding: 10px 12px;
  margin-top: 10px;
  font-size: 9.5px;
  line-height: 1.6;
  border-radius: 8px;
}

.config-warning {
  color: #93370d;
  background: #fffaeb;
  border: 1px solid #fedf89;
}

.config-warning .el-icon {
  flex: 0 0 auto;
  margin-top: 1px;
}

.backup-note {
  color: #475467;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
}

.backup-note .el-icon {
  flex: 0 0 auto;
  margin-top: 1px;
  color: #0f766e;
}

.backup-note code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #0f766e;
}

.source-heading {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  padding: 23px 3px 10px;
}

.section-kicker {
  display: block;
  margin-bottom: 3px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8px;
  font-weight: 650;
  letter-spacing: 1.7px;
  color: #8fb8b1;
}

.source-heading h2 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 14px;
  font-weight: 650;
  color: #344054;
}

.speed-all-button {
  display: flex;
  gap: 5px;
  align-items: center;
  padding: 6px 9px;
  font-size: 8.5px;
  color: #0f766e;
  cursor: pointer;
  background: #f0fdfa;
  border: 1px solid #d1f5ec;
  border-radius: 8px;
}

.speed-all-button:hover:not(:disabled) {
  background: #e4fbf5;
}

.speed-all-button:disabled {
  cursor: wait;
  opacity: 0.65;
}

.source-grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 11px;
}

.source-card {
  display: flex;
  flex-direction: column;
  min-height: 154px;
  padding: 15px;
  color: inherit;
  text-align: left;
  cursor: pointer;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 13px;
  box-shadow: 0 1px 0 rgba(255, 255, 255, 0.8) inset;
  transition:
    border-color 160ms ease,
    box-shadow 160ms ease,
    transform 160ms ease;
}

.source-card:hover {
  border-color: #a7e5da;
  box-shadow: 0 9px 24px rgba(15, 118, 110, 0.07);
  transform: translateY(-1px);
}

.source-card.selected {
  background:
    radial-gradient(circle at 100% 0%, rgba(45, 212, 191, 0.08), transparent 38%),
    #fff;
  border-color: #0f766e;
  box-shadow: 0 0 0 2px rgba(15, 118, 110, 0.07);
}

.source-card.current {
  border-color: #8edfd2;
}

.card-top {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 8px;
}

.source-name {
  font-size: 12px;
  font-weight: 650;
  color: #101828;
}

.source-badge {
  flex: 0 0 auto;
  padding: 3px 5px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 7.5px;
  font-weight: 650;
  letter-spacing: 0.4px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 4px;
}

.source-badge.custom {
  color: #6941c6;
  background: #f4f3ff;
}

.source-provider {
  display: block;
  margin-top: 3px;
  font-size: 8.5px;
  color: #98a2b3;
}

.source-card p {
  flex: 1;
  margin: 10px 0 9px;
  font-size: 9.5px;
  line-height: 1.55;
  color: #667085;
}

.source-speed-line {
  display: flex;
  gap: 7px;
  align-items: center;
  min-height: 21px;
  margin: 3px 0 7px;
  cursor: default;
}

.source-speed-button {
  display: flex;
  flex: 0 0 auto;
  gap: 3px;
  align-items: center;
  padding: 3px 6px;
  font-size: 7.5px;
  color: #0f766e;
  cursor: pointer;
  background: transparent;
  border: 1px solid #d1f5ec;
  border-radius: 6px;
}

.source-speed-button:hover:not(:disabled) {
  background: #f0fdfa;
}

.source-speed-button:disabled {
  cursor: wait;
  opacity: 0.6;
}

.source-speed-result {
  overflow: hidden;
  font-size: 7.5px;
  color: #0f766e;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.source-speed-result.failed {
  color: #b42318;
}

.spinning {
  animation: source-spin 1s linear infinite;
}

@keyframes source-spin {
  to {
    transform: rotate(360deg);
  }
}

.source-card code {
  display: block;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #9a8f84;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.card-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 18px;
  margin-top: 10px;
  padding-top: 9px;
  border-top: 1px solid #f1f3f5;
}

.current-label,
.select-label {
  display: flex;
  gap: 4px;
  align-items: center;
  font-size: 8.5px;
  color: #0f766e;
}

.current-label .el-icon {
  font-size: 10px;
}

.select-label {
  color: #b0b8c4;
}

.remove-custom {
  display: grid;
  width: 20px;
  height: 20px;
  padding: 0;
  font-size: 11px;
  color: #b0b8c4;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: 5px;
  place-items: center;
}

.remove-custom:hover {
  color: #d92d20;
  background: #fef3f2;
}

.add-card {
  cursor: default;
  background: #fbfcfd;
  border-style: dashed;
  border-color: #d7dee7;
}

.add-card:hover {
  border-color: #b8c4d2;
  box-shadow: none;
  transform: none;
}

.add-card-head {
  display: flex;
  gap: 9px;
  align-items: flex-start;
}

.add-icon {
  display: grid;
  flex: 0 0 26px;
  width: 26px;
  height: 26px;
  font-size: 12px;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 7px;
  place-items: center;
}

.add-card :deep(.el-input) {
  margin-top: 10px;
}

.add-card :deep(.el-input__wrapper) {
  min-height: 28px;
  background: #fff;
  border-radius: 7px;
  box-shadow: 0 0 0 1px #e4e7ec inset;
}

.add-card :deep(.el-input__inner) {
  font-size: 9.5px;
}

.add-source-button {
  height: 28px;
  padding: 0 9px;
  margin-top: 9px;
  font-size: 9.5px;
  border-radius: 7px;
}

.apply-panel {
  position: fixed;
  right: 30px;
  bottom: 16px;
  left: 316px;
  z-index: 20;
  display: flex;
  gap: 18px;
  align-items: center;
  justify-content: space-between;
  padding: 14px 17px;
  background: rgba(255, 255, 255, 0.94);
  border: 1px solid #dfece9;
  border-radius: 13px;
  box-shadow:
    0 14px 38px rgba(16, 24, 40, 0.13),
    0 0 0 1px rgba(15, 118, 110, 0.025);
  backdrop-filter: blur(14px);
}

.apply-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 2px;
}

.apply-label {
  font-size: 8.5px;
  color: #98a2b3;
}

.apply-copy strong {
  font-size: 12px;
  color: #101828;
}

.apply-copy p {
  max-width: 570px;
  margin: 0;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #98a2b3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.apply-actions {
  display: flex;
  flex: 0 0 auto;
  gap: 12px;
  align-items: center;
}

.backup-hint {
  display: flex;
  gap: 4px;
  align-items: center;
  font-size: 8.5px;
  color: #98a2b3;
}

.backup-hint .el-icon {
  font-size: 10px;
}

.apply-button {
  height: 33px;
  padding: 0 13px;
  font-size: 10.5px;
  border-radius: 8px;
}

.config-note {
  display: flex;
  gap: 7px;
  align-items: flex-start;
  padding: 11px 12px;
  margin-top: 12px;
  font-size: 9px;
  line-height: 1.65;
  color: #667085;
  background: #f8fafc;
  border: 1px solid #eef0f3;
  border-radius: 8px;
}

.config-note .el-icon {
  flex: 0 0 auto;
  margin-top: 1px;
  color: #98a2b3;
}

.config-note p {
  margin: 0;
}

.config-note code {
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8.5px;
  color: #475467;
}

.detected-section {
  padding: 21px 3px 0;
}

.detected-section h2 {
  margin: 0 0 8px;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 13px;
  color: #344054;
}

.detected-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.detected-list > div {
  display: flex;
  gap: 6px;
  align-items: center;
  max-width: 100%;
  padding: 6px 8px;
  background: #fff;
  border: 1px solid #e4e7ec;
  border-radius: 6px;
}

.detected-list span {
  font-size: 9px;
  font-weight: 600;
  color: #475467;
}

.detected-list code {
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 8px;
  color: #98a2b3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detected-list em {
  padding: 2px 4px;
  font-size: 7px;
  font-style: normal;
  color: #0f766e;
  background: #edf9f7;
  border-radius: 3px;
}

@keyframes page-in {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@keyframes spin-reverse {
  to {
    transform: rotate(-360deg);
  }
}

@media (max-width: 1180px) {
  .source-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .status-details {
    max-width: 260px;
  }

  .apply-panel {
    right: 22px;
    left: 308px;
  }
}

@media (max-width: 900px) {
  .source-hero {
    align-items: flex-start;
  }

  .source-grid {
    grid-template-columns: 1fr;
  }

  .status-panel {
    align-items: flex-start;
  }

  .status-details {
    display: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  .source-page,
  .loading-ring {
    animation: none;
  }
}
</style>

<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { ElMessage } from "element-plus";
import { Connection, Cpu, MagicStick } from "@element-plus/icons-vue";
import CargoSourcePage from "./CargoSourcePage.vue";
import RustToolchainPage from "./RustToolchainPage.vue";
import { operationBusy as busy } from "../composables/useOperationBusy";

const props = defineProps<{ rustInstalled: boolean }>();
const emit = defineEmits<{ "rust-state": [installed: boolean] }>();

type RustTab = "toolchain" | "source";
const activeTab = ref<RustTab>("toolchain");

const sourceAvailable = computed(() => props.rustInstalled);

watch(
  () => props.rustInstalled,
  (installed) => {
    if (!installed && activeTab.value === "source") {
      activeTab.value = "toolchain";
    }
  },
);

function selectTab(tab: RustTab) {
  if (tab === activeTab.value) {
    return;
  }
  if (tab === "source" && !sourceAvailable.value) {
    ElMessage.info("安装 Rust 工具链后即可管理 Cargo 下载源");
    return;
  }
  if (busy.value) {
    ElMessage.warning("Rust 操作进行中，请等待完成后再切换页面");
    return;
  }
  activeTab.value = tab;
}

function handleRustState(installed: boolean) {
  emit("rust-state", installed);
}
</script>

<template>
  <section class="rust-environment-page">
    <header class="env-hero">
      <div class="env-hero-copy">
        <span class="eyebrow"><i /> RUST ENVIRONMENT</span>
        <h1>Rust 环境，<span>一处配置完成。</span></h1>
        <p>工具链、下载源与存储设置集中管理，减少来回切换。</p>
      </div>
      <div class="env-hero-mark">
        <span class="env-orbit orbit-a" />
        <span class="env-orbit orbit-b" />
        <el-icon><MagicStick /></el-icon>
      </div>
    </header>

    <nav class="env-tabs" aria-label="Rust 环境功能">
      <button
        type="button"
        class="env-tab"
        :class="{ active: activeTab === 'toolchain' }"
        @click="selectTab('toolchain')"
      >
        <span class="env-tab-icon"><el-icon><Cpu /></el-icon></span>
        <span>
          <strong>工具链</strong>
          <small>版本、更新与存储</small>
        </span>
      </button>
      <button
        type="button"
        class="env-tab"
        :class="{ active: activeTab === 'source', disabled: !sourceAvailable }"
        :disabled="!sourceAvailable"
        @click="selectTab('source')"
      >
        <span class="env-tab-icon"><el-icon><Connection /></el-icon></span>
        <span>
          <strong>Cargo 下载源</strong>
          <small>crates.io 镜像与备份</small>
        </span>
      </button>
    </nav>

    <Transition name="env-panel" mode="out-in">
      <div :key="activeTab" class="env-panel">
        <RustToolchainPage
          v-if="activeTab === 'toolchain'"
          embedded
          @rust-state="handleRustState"
        />
        <CargoSourcePage v-else embedded />
      </div>
    </Transition>
  </section>
</template>

<style scoped>
.rust-environment-page {
  padding-top: 8px;
  animation: env-page-in 420ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.env-hero {
  position: relative;
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  min-height: 150px;
  padding: 25px 28px;
  margin-bottom: 14px;
  overflow: hidden;
  color: #fff7fc;
  background:
    radial-gradient(circle at 12% 0%, rgba(255, 255, 255, 0.18), transparent 34%),
    radial-gradient(circle at 88% 18%, rgba(56, 189, 248, 0.28), transparent 32%),
    linear-gradient(125deg, #3a1250 0%, #751c68 46%, #b02a6e 100%);
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 26px;
  box-shadow:
    0 24px 60px rgba(88, 28, 135, 0.25),
    0 1px 0 rgba(255, 255, 255, 0.23) inset;
  isolation: isolate;
}

.env-hero::before {
  position: absolute;
  inset: 0;
  z-index: -1;
  content: "";
  background:
    radial-gradient(circle at 70% 110%, rgba(255, 110, 160, 0.38), transparent 38%),
    radial-gradient(circle at 100% 0%, rgba(125, 211, 252, 0.18), transparent 40%);
  animation: env-aurora 9s ease-in-out infinite alternate;
}

.env-hero::after {
  position: absolute;
  right: -70px;
  bottom: -130px;
  width: 340px;
  height: 340px;
  content: "";
  border: 1px solid rgba(255, 255, 255, 0.11);
  border-radius: 50%;
  box-shadow:
    0 0 0 45px rgba(255, 255, 255, 0.035),
    0 0 0 95px rgba(255, 255, 255, 0.022);
}

.env-hero-copy {
  position: relative;
  z-index: 1;
  max-width: 650px;
}

.env-hero .eyebrow {
  display: flex;
  gap: 7px;
  align-items: center;
  margin-bottom: 10px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 700;
  letter-spacing: 2.2px;
  color: rgba(255, 255, 255, 0.72);
}

.env-hero .eyebrow i {
  width: 18px;
  height: 1px;
  background: #f9a8d4;
}

.env-hero h1 {
  margin: 0;
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: clamp(29px, 3vw, 39px);
  font-weight: 740;
  line-height: 1.16;
  letter-spacing: -1.3px;
  text-wrap: balance;
}

.env-hero h1 span {
  color: #fbcfe8;
}

.env-hero p {
  max-width: 560px;
  margin: 10px 0 0;
  font-size: 11.5px;
  line-height: 1.8;
  color: rgba(255, 255, 255, 0.7);
}

.env-hero-mark {
  position: relative;
  z-index: 1;
  display: grid;
  flex: 0 0 auto;
  width: 72px;
  height: 72px;
  margin: 0 8px 4px 20px;
  font-size: 28px;
  color: #fdf2f8;
  background: rgba(255, 255, 255, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 24px;
  box-shadow:
    0 16px 36px rgba(0, 0, 0, 0.18),
    0 1px 0 rgba(255, 255, 255, 0.24) inset;
  backdrop-filter: blur(16px);
  place-items: center;
  animation: env-mark-float 4.8s ease-in-out infinite;
}

.env-orbit {
  position: absolute;
  border: 1px solid rgba(255, 255, 255, 0.16);
  border-radius: 50%;
  pointer-events: none;
}

.orbit-a {
  inset: -12px;
}

.orbit-b {
  inset: -26px;
  border-color: rgba(255, 255, 255, 0.08);
}

.env-tabs {
  display: flex;
  gap: 7px;
  padding: 7px;
  margin-bottom: 12px;
  background: var(--surface-strong, rgba(255, 255, 255, 0.74));
  border: 1px solid var(--surface-line, rgba(139, 92, 246, 0.13));
  border-radius: 19px;
  box-shadow: var(--surface-shadow, 0 14px 40px rgba(76, 29, 149, 0.08));
  backdrop-filter: blur(20px);
}

.env-tab {
  position: relative;
  display: flex;
  gap: 10px;
  align-items: center;
  flex: 1;
  padding: 9px 14px;
  color: var(--app-muted, #7a6f8f);
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 14px;
  transition:
    color 180ms ease,
    background 180ms ease,
    border-color 180ms ease,
    transform 180ms ease;
}

.env-tab:hover:not(:disabled) {
  color: var(--app-ink, #241a35);
  background: rgba(255, 255, 255, 0.42);
}

.env-tab.active {
  color: #40133f;
  background:
    linear-gradient(110deg, rgba(255, 255, 255, 0.92), rgba(255, 241, 249, 0.8));
  border-color: rgba(236, 72, 153, 0.16);
  box-shadow:
    0 7px 20px rgba(190, 24, 93, 0.1),
    0 1px 0 rgba(255, 255, 255, 0.85) inset;
}

.env-tab.disabled {
  cursor: not-allowed;
  opacity: 0.48;
}

.env-tab-icon {
  display: grid;
  width: 31px;
  height: 31px;
  font-size: 14px;
  color: #a21caf;
  background: rgba(168, 85, 247, 0.09);
  border: 1px solid rgba(168, 85, 247, 0.1);
  border-radius: 10px;
  place-items: center;
}

.env-tab.active .env-tab-icon {
  color: #fff;
  background: linear-gradient(135deg, #db2777, #7c3aed);
  border-color: transparent;
  box-shadow: 0 7px 16px rgba(190, 24, 93, 0.24);
}

.env-tab > span:last-child {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.env-tab strong {
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.02em;
}

.env-tab small {
  font-size: 8px;
  color: var(--app-muted, #8c819f);
}

.env-panel {
  min-height: 180px;
}

.env-panel-enter-active,
.env-panel-leave-active {
  transition:
    opacity 220ms ease,
    transform 260ms cubic-bezier(0.2, 0.7, 0.2, 1),
    filter 220ms ease;
}

.env-panel-enter-from {
  opacity: 0;
  filter: blur(4px);
  transform: translateY(10px) scale(0.995);
}

.env-panel-leave-to {
  opacity: 0;
  filter: blur(3px);
  transform: translateY(-7px) scale(0.998);
}

@keyframes env-page-in {
  from {
    opacity: 0;
    transform: translateY(12px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes env-aurora {
  from {
    opacity: 0.75;
    transform: translate3d(-2%, 0, 0) scale(1);
  }
  to {
    opacity: 1;
    transform: translate3d(3%, -3%, 0) scale(1.08);
  }
}

@keyframes env-mark-float {
  0%,
  100% {
    transform: translateY(0) rotate(-2deg);
  }
  50% {
    transform: translateY(-6px) rotate(2deg);
  }
}

@media (max-width: 760px) {
  .env-hero {
    align-items: flex-start;
    padding: 22px;
  }

  .env-hero-mark {
    display: none;
  }

  .env-tabs {
    flex-direction: column;
  }
}

@media (prefers-reduced-motion: reduce) {
  .env-hero::before,
  .env-hero-mark,
  .rust-environment-page {
    animation: none;
  }
}
</style>

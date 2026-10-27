<script setup lang="ts">
import {
  Close,
  Connection,
  Cpu,
  FolderOpened,
  Grid,
  InfoFilled,
  Lock,
  Plus,
} from "@element-plus/icons-vue";
import { basename, cleanPath, formatBytes } from "../lib/format";

defineProps<{
  roots: string[];
  scanning: boolean;
  targetCount: number;
  totalBytes: number;
  version: string;
  updateAvailable: boolean;
  checkingUpdates: boolean;
  activePage: "targets" | "sources" | "toolchain";
  rustInstalled: boolean;
}>();

defineEmits<{
  add: [];
  remove: [root: string];
  "check-update": [];
  navigate: [page: "targets" | "sources" | "toolchain"];
}>();
</script>

<template>
  <aside class="sidebar">
    <div class="brand">
      <img class="brand-mark" src="/logo.svg" alt="" />
      <div class="brand-copy">
        <span class="brand-name">Cargo Sweep</span>
        <span class="brand-kicker">TARGET CLEANER</span>
      </div>
    </div>

    <nav class="page-nav" aria-label="主导航">
      <button
        class="nav-item"
        :class="{ active: activePage === 'targets' }"
        type="button"
        @click="$emit('navigate', 'targets')"
      >
        <span class="nav-icon"><el-icon><Grid /></el-icon></span>
        <span class="nav-title">空间清理</span>
        <span class="nav-hint">Targets</span>
      </button>
      <button
        v-if="rustInstalled"
        class="nav-item"
        :class="{ active: activePage === 'sources' }"
        type="button"
        @click="$emit('navigate', 'sources')"
      >
        <span class="nav-icon"><el-icon><Connection /></el-icon></span>
        <span class="nav-title">Cargo 下载源</span>
        <span class="nav-hint">Registry</span>
      </button>
      <button
        class="nav-item"
        :class="{ active: activePage === 'toolchain' }"
        type="button"
        @click="$emit('navigate', 'toolchain')"
      >
        <span class="nav-icon"><el-icon><Cpu /></el-icon></span>
        <span class="nav-title">Rust 工具链</span>
        <span class="nav-hint">Toolchain</span>
      </button>
    </nav>

    <div class="section-head">
      <span>扫描范围</span>
      <el-tooltip content="添加扫描目录" placement="right" :show-after="400">
        <button class="icon-button" type="button" :disabled="scanning" @click="$emit('add')">
          <el-icon><Plus /></el-icon>
        </button>
      </el-tooltip>
    </div>

    <div class="root-list">
      <div v-for="root in roots" :key="root" class="root-item">
        <div class="root-icon">
          <el-icon><FolderOpened /></el-icon>
        </div>
        <div class="root-copy">
          <span class="root-name">{{ basename(root) }}</span>
          <el-tooltip :content="cleanPath(root)" placement="bottom-start" :show-after="500">
            <span class="root-path">{{ cleanPath(root) }}</span>
          </el-tooltip>
        </div>
        <button
          class="root-remove"
          type="button"
          aria-label="移除扫描目录"
          :disabled="scanning"
          @click="$emit('remove', root)"
        >
          <el-icon><Close /></el-icon>
        </button>
      </div>

      <button v-if="roots.length === 0" class="empty-root" type="button" @click="$emit('add')">
        <span class="empty-root-plus"><el-icon><Plus /></el-icon></span>
        <span>添加 Rust 项目目录</span>
      </button>
    </div>

    <div class="depth-note">
      <el-icon><InfoFilled /></el-icon>
      <span>递归 10 层</span>
    </div>

    <div class="sidebar-spacer" />

    <div class="safety-card">
      <div class="safety-icon">
        <el-icon><Lock /></el-icon>
      </div>
      <div>
        <h4>只删 target</h4>
        <p>校验 Cargo.toml，跳过符号链接。</p>
      </div>
    </div>

    <div class="sidebar-footer">
      <div class="footer-stats">
        <span>{{ targetCount }} 个 target</span>
        <i />
        <span>{{ formatBytes(totalBytes) }}</span>
      </div>
      <button
        class="version"
        type="button"
        :disabled="checkingUpdates"
        @click="$emit('check-update')"
      >
        <span>v{{ version }}</span>
        <i v-if="updateAvailable" class="update-dot" />
        <span class="version-action">
          {{ checkingUpdates ? "检查中…" : updateAvailable ? "有新版本" : "检查更新" }}
        </span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  position: relative;
  display: flex;
  flex: 0 0 286px;
  flex-direction: column;
  min-height: 100vh;
  padding: 26px 18px 20px;
  overflow: hidden;
  color: #e7edf4;
  background:
    radial-gradient(circle at 15% 93%, rgba(45, 212, 191, 0.16), transparent 34%),
    radial-gradient(circle at 100% 0%, rgba(148, 233, 217, 0.08), transparent 26%),
    linear-gradient(160deg, #0a1120 0%, #0b1322 58%, #070c16 100%);
  border-right: 1px solid rgba(255, 255, 255, 0.06);
}

.sidebar::before {
  position: absolute;
  top: -90px;
  right: -110px;
  width: 260px;
  height: 260px;
  content: "";
  border: 1px solid rgba(255, 255, 255, 0.06);
  border-radius: 50%;
  box-shadow:
    0 0 0 38px rgba(255, 255, 255, 0.018),
    0 0 0 78px rgba(255, 255, 255, 0.012);
  pointer-events: none;
}

.brand {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 0 6px;
}

.brand-mark {
  width: 42px;
  height: 42px;
  border-radius: 12px;
  box-shadow: 0 10px 28px rgba(0, 0, 0, 0.34);
}

.brand-copy {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.brand-name {
  font-family: "Manrope Variable", "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 17px;
  font-weight: 700;
  letter-spacing: -0.2px;
}

.brand-kicker {
  margin-top: 2px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 9px;
  letter-spacing: 2.3px;
  color: #7f8ea3;
}

.page-nav {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 20px 4px 0;
}

.nav-item {
  display: flex;
  gap: 9px;
  align-items: center;
  width: 100%;
  padding: 9px 9px;
  color: #8595a9;
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 9px;
  transition:
    color 160ms ease,
    background 160ms ease,
    border-color 160ms ease;
}

.nav-item:hover {
  color: #d4f8f0;
  background: rgba(255, 255, 255, 0.035);
}

.nav-item.active {
  color: #ecfff9;
  background:
    linear-gradient(90deg, rgba(45, 212, 191, 0.14), rgba(45, 212, 191, 0.035));
  border-color: rgba(94, 234, 212, 0.1);
  box-shadow: inset 2px 0 0 #2dd4bf;
}

.nav-icon {
  display: grid;
  flex: 0 0 25px;
  width: 25px;
  height: 25px;
  font-size: 12px;
  color: #7f92a6;
  background: rgba(255, 255, 255, 0.045);
  border-radius: 7px;
  place-items: center;
}

.nav-item.active .nav-icon {
  color: #5eead4;
  background: rgba(45, 212, 191, 0.12);
}

.nav-title {
  flex: 1;
  font-size: 11px;
  font-weight: 600;
}

.nav-hint {
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 7.5px;
  letter-spacing: 0.5px;
  color: #586a7e;
}

.nav-item.active .nav-hint {
  color: #66a99f;
}

.section-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 8px 10px;
  margin-top: 12px;
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.7px;
  color: #c9d6e5;
  border-top: 1px solid rgba(255, 255, 255, 0.075);
}

.icon-button {
  display: grid;
  width: 25px;
  height: 25px;
  padding: 0;
  color: #aab8c8;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 7px;
  transition:
    color 160ms ease,
    background 160ms ease,
    transform 160ms ease;
  place-items: center;
}

.icon-button:hover:not(:disabled) {
  color: #fff;
  background: #0f766e;
  border-color: #14b8a6;
  transform: translateY(-1px);
}

.icon-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.root-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-height: min(38vh, 310px);
  padding: 2px 4px 4px;
  overflow-y: auto;
}

.root-item {
  display: flex;
  gap: 10px;
  align-items: center;
  padding: 10px 9px;
  background: rgba(255, 255, 255, 0.035);
  border: 1px solid rgba(255, 255, 255, 0.055);
  border-radius: 11px;
  transition:
    background 160ms ease,
    border-color 160ms ease;
}

.root-item:hover {
  background: rgba(255, 255, 255, 0.06);
  border-color: rgba(255, 255, 255, 0.09);
}

.root-icon {
  display: grid;
  flex: 0 0 30px;
  width: 30px;
  height: 30px;
  font-size: 14px;
  color: #5eead4;
  background: rgba(45, 212, 191, 0.14);
  border-radius: 8px;
  place-items: center;
}

.root-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
}

.root-name {
  overflow: hidden;
  font-size: 12.5px;
  font-weight: 600;
  color: #e7f0f7;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.root-path {
  display: block;
  margin-top: 2px;
  overflow: hidden;
  font-family: "Cascadia Code", "Consolas", monospace;
  font-size: 9.5px;
  color: #718197;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.root-remove {
  display: grid;
  flex: 0 0 22px;
  width: 22px;
  height: 22px;
  padding: 0;
  font-size: 12px;
  color: #8fa0b5;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: 6px;
  opacity: 0;
  transition:
    opacity 160ms ease,
    color 160ms ease,
    background 160ms ease;
  place-items: center;
}

.root-item:hover .root-remove,
.root-remove:focus-visible {
  opacity: 1;
}

.root-remove:hover:not(:disabled) {
  color: #fda29b;
  background: rgba(248, 113, 113, 0.12);
}

.root-remove:disabled {
  cursor: not-allowed;
}

.empty-root {
  display: flex;
  gap: 10px;
  align-items: center;
  justify-content: center;
  padding: 18px;
  font-size: 12px;
  color: #91a0b2;
  cursor: pointer;
  background: transparent;
  border: 1px dashed rgba(255, 255, 255, 0.15);
  border-radius: 11px;
  transition:
    color 160ms ease,
    border-color 160ms ease,
    background 160ms ease;
}

.empty-root:hover {
  color: #e9f7f4;
  background: rgba(255, 255, 255, 0.03);
  border-color: rgba(94, 234, 212, 0.5);
}

.empty-root-plus {
  display: grid;
  width: 24px;
  height: 24px;
  color: #5eead4;
  background: rgba(45, 212, 191, 0.12);
  border-radius: 7px;
  place-items: center;
}

.depth-note {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 12px 9px 0;
  font-size: 10.5px;
  color: #64748b;
}

.sidebar-spacer {
  flex: 1;
  min-height: 24px;
}

.safety-card {
  display: flex;
  gap: 11px;
  padding: 14px 13px;
  background: linear-gradient(145deg, rgba(255, 255, 255, 0.055), rgba(255, 255, 255, 0.022));
  border: 1px solid rgba(255, 255, 255, 0.07);
  border-radius: 13px;
}

.safety-icon {
  display: grid;
  flex: 0 0 27px;
  width: 27px;
  height: 27px;
  font-size: 13px;
  color: #2dd4bf;
  background: rgba(45, 212, 191, 0.12);
  border: 1px solid rgba(45, 212, 191, 0.18);
  border-radius: 8px;
  place-items: center;
}

.safety-card h4 {
  margin: 0;
  font-size: 11.5px;
  font-weight: 600;
  color: #d6e2ee;
}

.safety-card p {
  margin: 5px 0 0;
  font-size: 10.5px;
  line-height: 1.65;
  color: #7a8a9e;
}

.sidebar-footer {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 18px 8px 0;
}

.footer-stats {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 10.5px;
  color: #7b8a9e;
}

.footer-stats i {
  width: 2px;
  height: 2px;
  background: #46566b;
  border-radius: 50%;
}

.version {
  display: flex;
  gap: 6px;
  align-items: center;
  width: 100%;
  padding: 2px 0;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: 8.5px;
  font-weight: 600;
  letter-spacing: 1.1px;
  color: #5f7189;
  text-align: left;
  cursor: pointer;
  background: transparent;
  border: 0;
  transition: color 150ms ease;
}

.version:hover:not(:disabled) {
  color: #a7f3e4;
}

.version:disabled {
  cursor: wait;
}

.version-action {
  margin-left: auto;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 9px;
  font-weight: 500;
  letter-spacing: 0;
  color: #72849c;
}

.update-dot {
  width: 5px;
  height: 5px;
  background: #2dd4bf;
  border-radius: 50%;
  box-shadow: 0 0 0 3px rgba(45, 212, 191, 0.12);
}

@media (max-height: 740px) {
  .intro {
    padding: 18px 8px 16px;
    margin-top: 12px;
  }

  .intro p {
    display: none;
  }

  .safety-card {
    display: none;
  }
}
</style>

<script setup lang="ts">
import { computed } from "vue";
import { Clock, Coin, Files } from "@element-plus/icons-vue";
import {
  formatCount,
  formatDuration,
  formatRelativeTime,
  splitBytes,
} from "../lib/format";

const props = defineProps<{
  totalBytes: number;
  targetCount: number;
  fileCount: number;
  lastScanAt: number | null;
  lastScanDuration: number;
  scannedDirs: number;
  scannedRoots: number;
  scanning: boolean;
}>();

const amount = computed(() => splitBytes(props.totalBytes));
</script>

<template>
  <section class="summary-grid" aria-label="空间概况">
    <article class="stat-card stat-primary">
      <div class="stat-top">
        <span class="stat-label">可回收空间</span>
        <span class="stat-icon"><el-icon><Coin /></el-icon></span>
      </div>
      <div class="primary-amount">
        <span class="amount-value">{{ amount.value }}</span>
        <span class="amount-unit">{{ amount.unit }}</span>
      </div>
      <div class="stat-foot">
        <span class="foot-chip">{{ targetCount }} 个 target</span>
        <span class="foot-note">{{ scanning ? "正在更新…" : "等待释放" }}</span>
      </div>
      <div class="orbit orbit-one" />
      <div class="orbit orbit-two" />
      <div class="primary-glow" />
    </article>

    <article class="stat-card">
      <div class="stat-top">
        <span class="stat-label">构建产物</span>
        <span class="stat-icon neutral"><el-icon><Files /></el-icon></span>
      </div>
      <div class="secondary-amount">
        {{ formatCount(fileCount) }}
        <span>个文件</span>
      </div>
      <div class="stat-foot">
        <span class="foot-muted">已扫描 {{ formatCount(scannedDirs) }} 个目录</span>
      </div>
      <div class="accent-line" />
    </article>

    <article class="stat-card">
      <div class="stat-top">
        <span class="stat-label">上次扫描</span>
        <span class="stat-icon neutral"><el-icon><Clock /></el-icon></span>
      </div>
      <div class="secondary-amount relative">
        {{ scanning && !lastScanAt ? "扫描中…" : formatRelativeTime(lastScanAt) }}
      </div>
      <div class="stat-foot">
        <span class="foot-muted">
          {{
            lastScanAt
              ? `用时 ${formatDuration(lastScanDuration)} · ${scannedRoots} 个根目录`
              : scanning
                ? "正在读取项目目录"
                : "添加目录后开始第一次扫描"
          }}
        </span>
      </div>
      <div class="accent-line mint" />
    </article>
  </section>
</template>

<style scoped>
.summary-grid {
  display: grid;
  grid-template-columns: 1.22fr 1fr 1fr;
  gap: 14px;
}

.stat-card {
  position: relative;
  min-height: 152px;
  padding: 20px 21px 18px;
  overflow: hidden;
  background: #ffffff;
  border: 1px solid #e6eaef;
  border-radius: 17px;
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.9) inset,
    0 12px 32px rgba(16, 24, 40, 0.04);
  animation: rise 560ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.stat-card:nth-child(2) {
  animation-delay: 60ms;
}

.stat-card:nth-child(3) {
  animation-delay: 120ms;
}

.stat-primary {
  color: #f0fffb;
  background:
    radial-gradient(circle at 14% 5%, rgba(94, 234, 212, 0.2), transparent 38%),
    linear-gradient(135deg, #0f766e 0%, #0e6b64 46%, #083f3c 100%);
  border-color: rgba(15, 118, 110, 0.3);
  box-shadow:
    0 1px 0 rgba(255, 255, 255, 0.16) inset,
    0 18px 42px rgba(9, 76, 71, 0.2);
}

.stat-top {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.stat-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.4px;
  color: #667085;
}

.stat-primary .stat-label {
  color: rgba(231, 255, 249, 0.7);
}

.stat-icon {
  display: grid;
  width: 29px;
  height: 29px;
  font-size: 14px;
  color: #d8fff7;
  background: rgba(255, 255, 255, 0.11);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-radius: 9px;
  place-items: center;
}

.stat-icon.neutral {
  color: #5f6f80;
  background: #f7f9fb;
  border-color: #e9edf2;
}

.primary-amount {
  position: relative;
  z-index: 2;
  display: flex;
  gap: 6px;
  align-items: baseline;
  margin-top: 16px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-variant-numeric: tabular-nums;
}

.amount-value {
  font-size: clamp(36px, 3.5vw, 46px);
  font-weight: 620;
  line-height: 0.95;
  letter-spacing: -2px;
}

.amount-unit {
  font-size: 15px;
  font-weight: 550;
  color: rgba(214, 255, 246, 0.7);
}

.secondary-amount {
  margin-top: 21px;
  font-family: "Manrope Variable", "Segoe UI", sans-serif;
  font-size: clamp(26px, 2.45vw, 33px);
  font-weight: 620;
  line-height: 1;
  color: #101828;
  letter-spacing: -1.1px;
  font-variant-numeric: tabular-nums;
}

.secondary-amount span {
  margin-left: 4px;
  font-family: "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 12px;
  font-weight: 500;
  letter-spacing: 0;
  color: #98a2b3;
}

.secondary-amount.relative {
  font-family: "Segoe UI Variable Display", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: clamp(22px, 2vw, 28px);
  letter-spacing: -0.5px;
}

.stat-foot {
  position: absolute;
  right: 21px;
  bottom: 17px;
  left: 21px;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: space-between;
  min-height: 21px;
}

.foot-chip {
  padding: 4px 8px;
  font-family: "Manrope Variable", "Segoe UI", "Microsoft YaHei", sans-serif;
  font-size: 9.5px;
  font-weight: 600;
  color: #0b5f58;
  background: rgba(236, 253, 247, 0.9);
  border-radius: 6px;
}

.foot-note {
  font-size: 10px;
  color: rgba(214, 255, 246, 0.54);
}

.foot-muted {
  overflow: hidden;
  font-size: 10.5px;
  color: #98a2b3;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.accent-line {
  position: absolute;
  top: -13px;
  right: 24px;
  width: 80px;
  height: 80px;
  background: rgba(15, 118, 110, 0.035);
  border: 1px solid rgba(15, 118, 110, 0.06);
  border-radius: 24px;
  transform: rotate(18deg);
}

.accent-line.mint {
  background: rgba(45, 212, 191, 0.04);
  border-color: rgba(45, 212, 191, 0.07);
}

.orbit {
  position: absolute;
  z-index: 1;
  border: 1px solid rgba(224, 255, 249, 0.13);
  border-radius: 50%;
  pointer-events: none;
}

.orbit-one {
  top: -58px;
  right: -49px;
  width: 186px;
  height: 186px;
}

.orbit-two {
  top: -19px;
  right: -10px;
  width: 108px;
  height: 108px;
  border-color: rgba(224, 255, 249, 0.08);
}

.primary-glow {
  position: absolute;
  right: 38px;
  bottom: -46px;
  width: 130px;
  height: 100px;
  background: rgba(45, 212, 191, 0.14);
  filter: blur(28px);
}

@keyframes rise {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (max-width: 1140px) {
  .summary-grid {
    grid-template-columns: 1.1fr 0.9fr 0.9fr;
    gap: 12px;
  }

  .stat-card {
    padding: 17px;
  }

  .stat-foot {
    right: 17px;
    bottom: 15px;
    left: 17px;
  }
}

@media (prefers-reduced-motion: reduce) {
  .stat-card {
    animation: none;
  }
}
</style>

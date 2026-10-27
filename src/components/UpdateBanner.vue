<script setup lang="ts">
import { computed } from "vue";
import { ArrowRight, Close, Download } from "@element-plus/icons-vue";

const props = defineProps<{
  version: string;
  body?: string;
}>();

defineEmits<{
  open: [];
  dismiss: [];
}>();

const summary = computed(() => {
  const text = (props.body ?? "").replace(/[#*_`>-]/g, " ").replace(/\s+/g, " ").trim();
  if (!text) {
    return "包含体验优化与问题修复，建议更新到最新版本。";
  }
  return text.length > 82 ? `${text.slice(0, 82)}…` : text;
});
</script>

<template>
  <div class="update-banner">
    <div class="update-icon">
      <el-icon><Download /></el-icon>
    </div>
    <div class="update-copy">
      <strong>发现新版本 v{{ version }}</strong>
      <span>{{ summary }}</span>
    </div>
    <button class="update-action" type="button" @click="$emit('open')">
      查看更新
      <el-icon><ArrowRight /></el-icon>
    </button>
    <button class="update-close" type="button" aria-label="关闭更新提示" @click="$emit('dismiss')">
      <el-icon><Close /></el-icon>
    </button>
  </div>
</template>

<style scoped>
.update-banner {
  position: relative;
  display: flex;
  gap: 12px;
  align-items: center;
  padding: 12px 14px;
  overflow: hidden;
  background:
    radial-gradient(circle at 82% 0%, rgba(94, 234, 212, 0.2), transparent 35%),
    linear-gradient(100deg, #ecfdf9 0%, #f3fffc 58%, #ecfdf9 100%);
  border: 1px solid #bdf3e8;
  border-radius: 13px;
  box-shadow: 0 10px 30px rgba(15, 118, 110, 0.06);
  animation: slide-in 420ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
}

.update-banner::after {
  position: absolute;
  top: -20px;
  right: 160px;
  width: 90px;
  height: 90px;
  content: "";
  border: 1px solid rgba(15, 118, 110, 0.08);
  border-radius: 50%;
  pointer-events: none;
}

.update-icon {
  display: grid;
  flex: 0 0 33px;
  width: 33px;
  height: 33px;
  font-size: 15px;
  color: #0f766e;
  background: rgba(255, 255, 255, 0.66);
  border: 1px solid #b5eddf;
  border-radius: 9px;
  place-items: center;
}

.update-copy {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-width: 0;
  gap: 2px;
}

.update-copy strong {
  font-size: 11.5px;
  font-weight: 650;
  color: #115e59;
}

.update-copy span {
  overflow: hidden;
  font-size: 10px;
  color: #5f857e;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.update-action {
  display: flex;
  gap: 4px;
  align-items: center;
  padding: 6px 10px;
  font-size: 10px;
  font-weight: 600;
  color: #0f766e;
  cursor: pointer;
  background: rgba(255, 255, 255, 0.72);
  border: 1px solid #b5eddf;
  border-radius: 7px;
  transition:
    background 150ms ease,
    border-color 150ms ease;
}

.update-action:hover {
  background: #fff;
  border-color: #7fe0cf;
}

.update-action .el-icon {
  font-size: 10px;
}

.update-close {
  display: grid;
  width: 24px;
  height: 24px;
  padding: 0;
  font-size: 12px;
  color: #82a8a1;
  cursor: pointer;
  background: transparent;
  border: 0;
  border-radius: 6px;
  transition:
    color 150ms ease,
    background 150ms ease;
  place-items: center;
}

.update-close:hover {
  color: #115e59;
  background: rgba(15, 118, 110, 0.08);
}

@keyframes slide-in {
  from {
    opacity: 0;
    transform: translateY(-5px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@media (prefers-reduced-motion: reduce) {
  .update-banner {
    animation: none;
  }
}
</style>

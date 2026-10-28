import { computed, ref, watch } from "vue";

export type ThemeMode = "light" | "dark" | "system";
export type ColorPalette =
  | "rose"
  | "mint"
  | "ocean"
  | "sunset"
  | "forest"
  | "graphite";

export interface PaletteOption {
  id: ColorPalette;
  name: string;
  description: string;
  colors: [string, string, string];
}

export const colorPalettes: PaletteOption[] = [
  {
    id: "rose",
    name: "玫瑰星云",
    description: "玫红 · 紫罗兰",
    colors: ["#db2777", "#8b5cf6", "#22d3ee"],
  },
  {
    id: "mint",
    name: "薄荷浪潮",
    description: "青绿 · 海蓝",
    colors: ["#0d9488", "#0ea5e9", "#a3e635"],
  },
  {
    id: "ocean",
    name: "深海极光",
    description: "钴蓝 · 靛紫",
    colors: ["#2563eb", "#7c3aed", "#06b6d4"],
  },
  {
    id: "sunset",
    name: "落日熔金",
    description: "橙金 · 珊瑚",
    colors: ["#ea580c", "#e11d48", "#f59e0b"],
  },
  {
    id: "forest",
    name: "森野呼吸",
    description: "翠绿 · 青柠",
    colors: ["#16a34a", "#0d9488", "#84cc16"],
  },
  {
    id: "graphite",
    name: "石墨金线",
    description: "石墨 · 鎏金",
    colors: ["#475569", "#a16207", "#f59e0b"],
  },
];

const STORAGE_KEY = "cargo-sweep.theme";
const PALETTE_STORAGE_KEY = "cargo-sweep.palette";

function readMode(): ThemeMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "light" || stored === "dark" || stored === "system") {
      return stored;
    }
  } catch {
    // Fall through to the system preference.
  }
  return "system";
}

function readPalette(): ColorPalette {
  try {
    const stored = localStorage.getItem(PALETTE_STORAGE_KEY);
    if (colorPalettes.some((palette) => palette.id === stored)) {
      return stored as ColorPalette;
    }
  } catch {
    // Fall through to the default palette.
  }
  return "rose";
}

const media =
  typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;
const systemDark = ref(media?.matches ?? false);
const mode = ref<ThemeMode>(readMode());
const palette = ref<ColorPalette>(readPalette());

export const isDark = computed(
  () => mode.value === "dark" || (mode.value === "system" && systemDark.value),
);

watch(
  isDark,
  (dark) => {
    if (typeof document === "undefined") {
      return;
    }
    document.documentElement.classList.toggle("dark", dark);
    document.documentElement.style.colorScheme = dark ? "dark" : "light";
  },
  { immediate: true },
);

watch(
  palette,
  (nextPalette) => {
    if (typeof document === "undefined") {
      return;
    }
    document.documentElement.dataset.palette = nextPalette;
  },
  { immediate: true },
);

media?.addEventListener("change", (event) => {
  systemDark.value = event.matches;
});

function setMode(nextMode: ThemeMode) {
  mode.value = nextMode;
  try {
    localStorage.setItem(STORAGE_KEY, nextMode);
  } catch {
    // Theme selection remains active for the current session.
  }
}

function setPalette(nextPalette: ColorPalette) {
  palette.value = nextPalette;
  try {
    localStorage.setItem(PALETTE_STORAGE_KEY, nextPalette);
  } catch {
    // Palette selection remains active for the current session.
  }
}

export function useTheme() {
  return {
    mode,
    palette,
    isDark,
    setMode,
    setPalette,
  };
}

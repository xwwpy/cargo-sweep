import { computed, ref, watch } from "vue";

export type ThemeMode = "light" | "dark" | "system";

const STORAGE_KEY = "cargo-sweep.theme";

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

const media =
  typeof window !== "undefined"
    ? window.matchMedia("(prefers-color-scheme: dark)")
    : null;
const systemDark = ref(media?.matches ?? false);
const mode = ref<ThemeMode>(readMode());

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

export function useTheme() {
  return {
    mode,
    isDark,
    setMode,
  };
}

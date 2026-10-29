import { createApp } from "vue";
import { invoke } from "@tauri-apps/api/core";
import ElementPlus from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "@fontsource-variable/manrope/wght.css";
import "element-plus/dist/index.css";
import "element-plus/theme-chalk/dark/css-vars.css";
import "./styles.css";
import App from "./App.vue";
import "./theme-dark.css";
import "./styles-refresh.css";

function describeError(error: unknown): string {
  if (error instanceof Error) {
    return error.message || error.name;
  }
  return typeof error === "string" ? error : "未知界面错误";
}

function showFatalOverlay(message: string) {
  if (document.getElementById("fatal-overlay")) {
    return;
  }
  const overlay = document.createElement("div");
  overlay.id = "fatal-overlay";
  overlay.style.cssText =
    "position:fixed;inset:0;z-index:99999;display:grid;place-items:center;padding:24px;" +
    "background:#f7f3fd;color:#241832;font-family:Segoe UI,sans-serif;text-align:center;";
  const panel = document.createElement("div");
  panel.style.cssText =
    "max-width:360px;padding:26px;border:1px solid #eadff3;border-radius:16px;background:#fff;box-shadow:0 20px 50px rgba(40,10,60,.14);";
  const title = document.createElement("h2");
  title.textContent = "界面遇到问题";
  title.style.cssText = "margin:0 0 8px;font-size:17px;";
  const detail = document.createElement("p");
  detail.textContent = message;
  detail.style.cssText = "margin:0 0 14px;font-size:11px;line-height:1.6;color:#7d6f92;";
  const reload = document.createElement("button");
  reload.type = "button";
  reload.textContent = "重新加载";
  reload.style.cssText =
    "padding:7px 14px;border:0;border-radius:8px;background:#db2777;color:#fff;font-size:11px;cursor:pointer;";
  reload.addEventListener("click", () => location.reload());
  panel.append(title, detail, reload);
  overlay.append(panel);
  document.body.append(overlay);
}

function reportFatalError(error: unknown) {
  const message = describeError(error);
  console.error("Cargo Sweep failed to render:", error);
  window.__cargoSweepBootError?.(`界面加载失败：${message}`);
  showFatalOverlay(message);
  try {
    void invoke("log_frontend_error", { message }).catch(() => {});
  } catch {
    // Logging must never mask the original failure.
  }
}

window.addEventListener("error", (event) => {
  if (!window.__cargoSweepBooted) {
    reportFatalError(event.error ?? event.message);
  }
});
window.addEventListener("unhandledrejection", (event) => {
  if (!window.__cargoSweepBooted) {
    reportFatalError(event.reason);
  }
});

const app = createApp(App);
app.config.errorHandler = (error) => {
  reportFatalError(error);
};

app.use(ElementPlus, { locale: zhCn });

try {
  app.mount("#app");
  window.setTimeout(() => {
    if (!document.querySelector(".app-shell")) {
      reportFatalError(new Error("页面根节点未渲染"));
      return;
    }
    window.__cargoSweepMarkReady?.();
    try {
      void invoke("show_main_window").catch(() => {});
    } catch {
      // Browser previews have no Tauri bridge.
    }
  }, 40);
} catch (error) {
  reportFatalError(error);
}

import { createApp } from "vue";
import ElementPlus from "element-plus";
import zhCn from "element-plus/es/locale/lang/zh-cn";
import "@fontsource-variable/manrope/wght.css";
import "element-plus/dist/index.css";
import "./styles.css";
import App from "./App.vue";

createApp(App).use(ElementPlus, { locale: zhCn }).mount("#app");

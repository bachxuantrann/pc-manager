import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { applyTheme, loadThemePref } from "./lib/theme";

async function start() {
  // Áp giao diện đã chọn trước khi vẽ (index.html mặc định data-theme="dark").
  applyTheme(loadThemePref());
  // Chế độ demo (dữ liệu giả, để chụp ảnh README) — chỉ có trong bản dev, bị loại khỏi bản build.
  if (import.meta.env.DEV && new URLSearchParams(location.search).has("demo")) {
    await import("./dev/demo");
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void start();

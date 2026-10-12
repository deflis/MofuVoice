import { createRoot } from "react-dom/client";
import { App } from "./App";
import "./styles.css";
const root = document.getElementById("root");
if (!root) {
  throw new Error("画面の描画先 #root が見つかりません。");
}
createRoot(root).render(<App />);

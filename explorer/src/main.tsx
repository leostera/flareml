import { createRoot } from "react-dom/client";
import "@xyflow/react/dist/style.css";
import "./style.css";
import "./debugger.css";
import App from "./App";
import { nativeProvider } from "./providers/native";
createRoot(document.getElementById("root")!).render(
  <App provider={nativeProvider()} />,
);

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { loadSavedTheme } from "./themes";

loadSavedTheme();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

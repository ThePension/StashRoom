import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { initWatchEvents } from "./lib/watchEvents";

// Initialize global watch event listener
initWatchEvents();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

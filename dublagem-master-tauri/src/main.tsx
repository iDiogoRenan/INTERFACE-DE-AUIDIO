import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { installFrontendFatalErrorReporter } from "./shared/errors/fatalErrorReporter";
import "./styles/theme.css";

installFrontendFatalErrorReporter();

const root = document.getElementById("root");

if (root === null) {
  throw new Error("Elemento raiz não encontrado.");
}

ReactDOM.createRoot(root).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);

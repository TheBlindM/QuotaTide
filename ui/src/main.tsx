import { render } from "preact";

import { App } from "./App";
import { I18nProvider } from "./i18n-context";
import "./styles.css";

const root = document.getElementById("root");

if (root === null) {
  throw new Error("QuotaTide root element is missing");
}

render(
  <I18nProvider preference="system">
    <App />
  </I18nProvider>,
  root,
);

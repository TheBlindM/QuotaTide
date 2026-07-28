import { render } from "preact";

import { App } from "./App";
import "./styles.css";

const root = document.getElementById("root");

if (root === null) {
  throw new Error("QuotaTide root element is missing");
}

render(<App />, root);

import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import ChatWindow from "./ChatWindow";
import PlayPanel, { Toy } from "./PlayPanel";
const view = new URLSearchParams(location.search).get("view");
import "./styles.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {view === "chat" ? <ChatWindow /> : view === "playground" ? <PlayPanel /> : view === "toy" ? <Toy /> : <App />}
  </React.StrictMode>
);

import React from "react";
import ReactDOM from "react-dom/client";

const view = window.location.hash.replace("#", "");

async function boot() {
  await Promise.all([
    import("@fontsource-variable/inter/index.css"),
    import("@fontsource/newsreader/400.css"),
    import("@fontsource/newsreader/500.css"),
  ]);
  await import("./app/app.css");
  const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
  if (view === "popup") {
    document.body.classList.add("overlay");
    const { default: Popup } = await import("./app/Popup");
    root.render(<Popup />);
    return;
  }
  if (view === "quick") {
    document.body.classList.add("overlay");
    const { default: Quick } = await import("./app/Quick");
    root.render(<Quick />);
    return;
  }
  if (view === "pill") {
    document.body.classList.add("overlay");
    const { default: Pill } = await import("./app/Pill");
    root.render(<Pill />);
    return;
  }
  const { default: App } = await import("./app/App");
  const { default: ErrorBoundary } = await import("./app/ErrorBoundary");
  root.render(
    <React.StrictMode>
      <ErrorBoundary>
        <App />
      </ErrorBoundary>
    </React.StrictMode>,
  );
  // Show the window only after the first paint, so it never flashes white.
  requestAnimationFrame(() => requestAnimationFrame(() => import("./api").then(({ api }) => api.uiReady().catch(() => {}))));
}
boot();

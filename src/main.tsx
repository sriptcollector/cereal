import React from "react";
import ReactDOM from "react-dom/client";

const view = window.location.hash.replace("#", "");

async function boot() {
  await Promise.all([
    import("@fontsource/space-grotesk/400.css"),
    import("@fontsource/space-grotesk/500.css"),
    import("@fontsource/space-grotesk/600.css"),
    import("@fontsource/space-grotesk/700.css"),
    import("@fontsource/space-mono/400.css"),
  ]);
  await import("./theme/toolbay-theme.css");
  await import("./app/app.css");
  const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
  if (view === "popup") {
    document.body.classList.add("overlay");
    const { default: Popup } = await import("./app/Popup");
    root.render(<Popup />);
    return;
  }
  if (view === "pill") {
    document.body.classList.add("overlay");
    const { default: Pill } = await import("./app/Pill");
    root.render(<Pill />);
    return;
  }
  const { default: App } = await import("./app/App");
  root.render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
  // Show the window only after the first paint, so it never flashes white.
  requestAnimationFrame(() => requestAnimationFrame(() => import("./api").then(({ api }) => api.uiReady().catch(() => {}))));
}
boot();

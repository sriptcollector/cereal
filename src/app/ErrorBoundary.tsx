import { Component, ReactNode } from "react";

/** Never leave a blank window: show what broke and a way out. */
export default class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null; stack: string }> {
  state = { error: null as Error | null, stack: "" };
  static getDerivedStateFromError(error: Error) {
    return { error };
  }
  componentDidCatch(error: Error, info: { componentStack?: string | null }) {
    console.error(error);
    this.setState({ stack: `${error.stack ?? ""}\n${info.componentStack ?? ""}` });
  }
  render() {
    if (!this.state.error) return this.props.children;
    const err = this.state.error as Error | null;
    return (
      <div className="crash">
        <h2>Something went wrong on this page</h2>
        <p className="muted">Your notes and recordings are safe. Reload to continue.</p>
        <pre>{String(err?.message ?? err)}</pre>
        {this.state.stack && <pre className="small">{this.state.stack.split(/\r?\n/).slice(0, 14).join("\n")}</pre>}
        <button className="btn primary" onClick={() => location.reload()}>
          Reload
        </button>
      </div>
    );
  }
}

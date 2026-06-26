import { Component, type ErrorInfo, type ReactNode } from "react";

interface ErrorBoundaryProps {
    children: ReactNode;
}

interface ErrorBoundaryState {
    error: Error | null;
}

/**
 * Top-level error boundary.
 *
 * A render-time throw anywhere in the tree (for example a malformed config
 * value reaching a component) would otherwise unmount the whole app and leave
 * the webview blank — a dark overlay on macOS WKWebView, a white screen on
 * Windows WebView2. This boundary catches the throw and renders a recoverable
 * fallback instead, so a localized bug can no longer take down the entire UI.
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
    constructor(props: ErrorBoundaryProps) {
        super(props);
        this.state = { error: null };
    }

    static getDerivedStateFromError(error: Error): ErrorBoundaryState {
        return { error };
    }

    componentDidCatch(error: Error, info: ErrorInfo): void {
        // Keep the stack in the devtools console for diagnosis; the fallback UI
        // intentionally stays terse.
        console.error("Unhandled UI error:", error, info.componentStack);
    }

    private handleReload = (): void => {
        this.setState({ error: null });
        window.location.reload();
    };

    render(): ReactNode {
        const { error } = this.state;
        if (!error) {
            return this.props.children;
        }

        return (
            <div
                style={{
                    display: "flex",
                    flexDirection: "column",
                    alignItems: "center",
                    justifyContent: "center",
                    gap: 16,
                    height: "100vh",
                    padding: 40,
                    textAlign: "center",
                    color: "var(--text-2)",
                    background: "var(--bg)",
                }}
            >
                <div style={{ fontSize: 16, fontWeight: 600, color: "var(--text-1)" }}>
                    Something went wrong
                </div>
                <div
                    style={{
                        fontSize: 13,
                        maxWidth: 420,
                        color: "var(--text-3)",
                        wordBreak: "break-word",
                    }}
                >
                    {error.message}
                </div>
                <button
                    type="button"
                    className="btn btn-primary"
                    onClick={this.handleReload}
                    style={{ marginTop: 8 }}
                >
                    Reload
                </button>
            </div>
        );
    }
}

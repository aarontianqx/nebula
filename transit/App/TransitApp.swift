import SwiftUI
import TransitCore

@main
struct TransitApp: App {
    @StateObject private var model = AppModel()
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        MenuBarExtra {
            MainPanelView()
                .environmentObject(model)
                .onAppear {
                    appDelegate.model = model
                    model.start()
                }
        } label: {
            MenuBarLabel(status: model.proxyStatus, summary: model.summaries[.today])
                .onAppear {
                    appDelegate.model = model
                    model.start()
                }
        }
        .menuBarExtraStyle(.window)
    }
}

private struct MenuBarLabel: View {
    let status: ProxyStatus
    let summary: UsageSummary?

    var body: some View {
        HStack(spacing: 4) {
            Image(systemName: icon)
            if let summary, summary.totalTokens > 0 {
                Text(formatTokens(summary.totalTokens))
            }
        }
    }

    private var icon: String {
        switch status.state {
        case .running: "arrow.left.arrow.right.circle.fill"
        case .starting: "clock.arrow.circlepath"
        case .degraded: "exclamationmark.triangle.fill"
        case .stopped: "arrow.left.arrow.right.circle"
        }
    }
}

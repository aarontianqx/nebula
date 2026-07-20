import AppKit

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    weak var model: AppModel?
    private var terminationPending = false
    private var terminationConfirmed = false
    private var systemIsTerminating = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSWorkspace.shared.notificationCenter.addObserver(
            self,
            selector: #selector(workspaceWillPowerOff),
            name: NSWorkspace.willPowerOffNotification,
            object: nil
        )
    }

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminationPending, let model else { return .terminateNow }
        if !systemIsTerminating, !terminationConfirmed, model.hasEnabledRoutes {
            let alert = NSAlert()
            alert.alertStyle = .warning
            alert.messageText = "退出 Transit？"
            alert.informativeText = "退出后本机代理会停止，仍指向 Transit 的 Agent 请求将无法连接。"
            alert.addButton(withTitle: "退出")
            alert.addButton(withTitle: "取消")
            guard alert.runModal() == .alertFirstButtonReturn else { return .terminateCancel }
            terminationConfirmed = true
        }
        terminationPending = true
        Task {
            await model.shutdown(gracePeriod: systemIsTerminating ? .zero : .seconds(10))
            sender.reply(toApplicationShouldTerminate: true)
        }
        return .terminateLater
    }

    @objc private func workspaceWillPowerOff() {
        systemIsTerminating = true
    }
}

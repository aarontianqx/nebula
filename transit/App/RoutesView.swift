import AppKit
import SwiftUI
import TransitCore

struct RoutesView: View {
    @EnvironmentObject private var model: AppModel
    @State private var editingRoute: RouteConfiguration?

    var body: some View {
        ZStack {
            routeList

            if let route = editingRoute {
                // Keep the editor in the MenuBarExtra window. A sheet creates a
                // second transient window; when a text field becomes key,
                // macOS can dismiss the parent menu-bar window and its sheet.
                Color.black.opacity(0.12)
                    .ignoresSafeArea()
                RouteEditorView(
                    route: route,
                    onCancel: { editingRoute = nil },
                    onSaved: { editingRoute = nil }
                )
                .environmentObject(model)
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 16))
                .clipShape(RoundedRectangle(cornerRadius: 16))
                .padding(10)
                .shadow(color: .black.opacity(0.18), radius: 18, y: 8)
            }
        }
    }

    private var routeList: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Toggle("代理运行", isOn: Binding(
                    get: { model.proxyStatus.state == .running || model.proxyStatus.state == .degraded },
                    set: { running in Task { await model.setProxyRunning(running) } }
                ))
                .toggleStyle(.switch)
                .controlSize(.small)
                Spacer()
                Button {
                    let suffix = UUID().uuidString.prefix(6).lowercased()
                    editingRoute = RouteConfiguration(
                        displayName: "New Route",
                        agentID: "coding-agent",
                        listener: ListenerConfiguration(port: 8787, pathPrefix: "/route-\(suffix)"),
                        upstream: ""
                    )
                } label: {
                    Label("添加 Route", systemImage: "plus")
                }
            }
            if model.configuration.routes.isEmpty {
                ContentUnavailableView {
                    Label("尚未配置 Route", systemImage: "point.3.connected.trianglepath.dotted")
                } description: {
                    Text("添加本地入口、协议和用户自己的上游地址。")
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                List(model.configuration.routes) { route in
                    HStack(spacing: 10) {
                        Image(systemName: route.enabled ? "arrow.left.arrow.right.circle.fill" : "pause.circle")
                            .foregroundStyle(route.enabled ? .blue : .secondary)
                        VStack(alignment: .leading, spacing: 3) {
                            HStack {
                                Text(route.displayName).font(.callout.weight(.medium))
                                Text(route.protocolType.displayName)
                                    .font(.caption2)
                                    .padding(.horizontal, 5)
                                    .padding(.vertical, 1)
                                    .background(.quaternary, in: Capsule())
                            }
                            Text(route.localBaseURL)
                                .font(.caption2.monospaced())
                                .foregroundStyle(.secondary)
                            Text(routeStatus(route))
                                .font(.caption2)
                                .foregroundStyle(routeStatusColor(route))
                        }
                        Spacer()
                        if let result = model.routeTestResults[route.id] {
                            Text(result).font(.caption2).foregroundStyle(.secondary).lineLimit(1)
                        }
                        Menu {
                            Button("编辑") { editingRoute = route }
                            Button("复制 Base URL") {
                                NSPasteboard.general.clearContents()
                                NSPasteboard.general.setString(route.localBaseURL, forType: .string)
                            }
                            Button("测试上游连接") { Task { await model.testRoute(route) } }
                            Divider()
                            Button("删除", role: .destructive) {
                                Task {
                                    let secretReference = route.authentication.secretRef
                                    var config = model.configuration
                                    config.routes.removeAll { $0.id == route.id }
                                    if await model.applyConfiguration(config),
                                       let secretReference,
                                       !config.routes.contains(where: {
                                           $0.authentication.secretRef == secretReference
                                       }) {
                                        await model.deleteSecret(reference: secretReference)
                                    }
                                }
                            }
                        } label: {
                            Image(systemName: "ellipsis.circle")
                        }
                        .menuStyle(.borderlessButton)
                    }
                }
                .listStyle(.inset)
            }
        }
        .padding(14)
    }

    private func routeStatus(_ route: RouteConfiguration) -> String {
        guard route.enabled else { return "已停用" }
        if model.configurationIssues.contains(where: { $0.routeID == route.id && $0.severity == .error }) {
            return "配置无效"
        }
        if let listener = model.proxyStatus.listeners.first(where: { $0.port == route.listener.port }) {
            switch listener.state {
            case .binding: return "正在绑定"
            case .failed: return "Listener 异常"
            case .ready: break
            }
        } else {
            return "未运行"
        }
        if let latest = model.recentEvents.first(where: { $0.routeID == route.id }),
           latest.outcome == .failed,
           latest.errorCode?.hasPrefix("upstream_") == true {
            return "上游异常"
        }
        return "就绪"
    }

    private func routeStatusColor(_ route: RouteConfiguration) -> Color {
        switch routeStatus(route) {
        case "就绪": .green
        case "正在绑定": .blue
        case "已停用", "未运行": .secondary
        default: .orange
        }
    }
}

private struct RouteEditorView: View {
    @EnvironmentObject private var model: AppModel
    @State var route: RouteConfiguration
    let onCancel: () -> Void
    let onSaved: () -> Void
    @State private var secretInput = ""
    @State private var saving = false

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text(model.configuration.routes.contains(where: { $0.id == route.id }) ? "编辑 Route" : "添加 Route")
                .font(.title3.weight(.semibold))
            Form {
                TextField("名称", text: $route.displayName)
                TextField("Agent 标签", text: $route.agentID)
                HStack {
                    TextField("本地端口", value: $route.listener.port, format: .number)
                    TextField("路径前缀", text: $route.listener.pathPrefix)
                }
                TextField("上游 URL", text: $route.upstream)
                if route.upstream.lowercased().hasPrefix("http://") {
                    Toggle("允许不安全的 HTTP 上游", isOn: $route.allowInsecureHTTP)
                }
                Picker("协议", selection: $route.protocolType) {
                    ForEach(UsageProtocol.allCases) { Text($0.displayName).tag($0) }
                }
                Picker("认证", selection: $route.authentication.mode) {
                    Text("透传客户端认证").tag(AuthenticationMode.passthrough)
                    Text("注入 Bearer Token").tag(AuthenticationMode.replaceBearer)
                    Text("注入自定义 Header").tag(AuthenticationMode.replaceHeader)
                }
                if route.authentication.mode != .passthrough {
                    Label(
                        "Relay 模式：Transit 将从 Keychain 读取并代表客户端注入 credential。",
                        systemImage: "exclamationmark.shield"
                    )
                    .font(.caption)
                    .foregroundStyle(.orange)
                    if route.authentication.mode == .replaceHeader {
                        TextField("Header 名称", text: Binding(
                            get: { route.authentication.headerName ?? "x-api-key" },
                            set: { route.authentication.headerName = $0 }
                        ))
                    }
                    SecureField(route.authentication.secretRef == nil ? "Secret" : "新 Secret（留空则不修改）", text: $secretInput)
                }
                if !model.configuration.pricingPolicies.isEmpty {
                    Picker("价格规则", selection: $route.pricingPolicyID) {
                        Text("不估算成本").tag(nil as String?)
                        ForEach(model.configuration.pricingPolicies) { Text($0.id).tag($0.id as String?) }
                    }
                }
                Toggle("启用", isOn: $route.enabled)
            }
            HStack {
                Button("取消") { onCancel() }
                Spacer()
                Button("保存") { save() }
                    .keyboardShortcut(.defaultAction)
                    .disabled(saving)
            }
        }
        .padding(18)
        .frame(width: 460)
    }

    private func save() {
        saving = true
        Task {
            var newlyCreatedSecretReference: String?
            let previousSecretReference = route.authentication.secretRef
            do {
                if route.authentication.mode != .passthrough {
                    if route.authentication.mode == .replaceHeader,
                       route.authentication.headerName?.isEmpty != false {
                        route.authentication.headerName = "x-api-key"
                    }
                    if route.authentication.mode == .replaceBearer {
                        route.authentication.headerName = nil
                    }
                    if !secretInput.isEmpty {
                        let reference = try model.saveSecret(secretInput)
                        newlyCreatedSecretReference = reference
                        route.authentication.secretRef = reference
                    }
                } else {
                    route.authentication.secretRef = nil
                    route.authentication.headerName = nil
                }
                var config = model.configuration
                if let index = config.routes.firstIndex(where: { $0.id == route.id }) {
                    config.routes[index] = route
                } else {
                    config.routes.append(route)
                }
                if await model.applyConfiguration(config) {
                    if let previousSecretReference,
                       previousSecretReference != route.authentication.secretRef,
                       !model.configuration.routes.contains(where: {
                           $0.authentication.secretRef == previousSecretReference
                       }) {
                        await model.deleteSecret(reference: previousSecretReference)
                    }
                    onSaved()
                } else if let newlyCreatedSecretReference {
                    await model.deleteSecret(reference: newlyCreatedSecretReference)
                }
            } catch {
                if let newlyCreatedSecretReference {
                    await model.deleteSecret(reference: newlyCreatedSecretReference)
                }
                model.report(error)
            }
            saving = false
        }
    }
}

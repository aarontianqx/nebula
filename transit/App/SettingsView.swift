import SwiftUI
import TransitCore
import TransitShared

struct SettingsView: View {
    @EnvironmentObject private var model: AppModel
    @State private var editingPolicy: PricingPolicy?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                GroupBox("启动") {
                    Toggle("登录时启动 Transit", isOn: Binding(
                        get: { model.launchAtLoginEnabled },
                        set: { model.setLaunchAtLogin($0) }
                    ))
                    .padding(.vertical, 4)
                }
                GroupBox("数据") {
                    Stepper(
                        "保留 \(model.configuration.storage.retentionDays) 天",
                        value: Binding(
                            get: { model.configuration.storage.retentionDays },
                            set: { days in
                                Task {
                                    var config = model.configuration
                                    config.storage.retentionDays = days
                                    _ = await model.applyConfiguration(config)
                                }
                            }
                        ),
                        in: 1...3650
                    )
                    .padding(.vertical, 4)
                }
                GroupBox("Widget") {
                    Picker("主指标", selection: Binding(
                        get: { model.widgetPrimaryMetric },
                        set: { model.setWidgetPrimaryMetric($0) }
                    )) {
                        ForEach(WidgetPrimaryMetric.allCases) { metric in
                            Text(metric.displayName).tag(metric)
                        }
                    }
                    .padding(.vertical, 4)
                }
                GroupBox("Credentials") {
                    VStack(alignment: .leading, spacing: 8) {
                        Text("这里只显示 Transit 管理的 Keychain 项，不会读取或回显 credential。")
                            .font(.caption)
                            .foregroundStyle(.secondary)
                        if model.keychainSecretReferences.isEmpty {
                            Text("暂无 credential 引用")
                                .foregroundStyle(.secondary)
                        } else {
                            ForEach(model.keychainSecretReferences, id: \.self) { reference in
                                let isReferenced = model.configuration.routes.contains {
                                    $0.authentication.secretRef == reference
                                }
                                HStack {
                                    VStack(alignment: .leading, spacing: 2) {
                                        Text(reference).font(.callout.monospaced())
                                        if isReferenced {
                                            Text("被 Route 引用；请先编辑或删除对应 Route")
                                                .font(.caption2)
                                                .foregroundStyle(.secondary)
                                        }
                                    }
                                    Spacer()
                                    Button("从 Keychain 删除", role: .destructive) {
                                        Task { await model.deleteSecret(reference: reference) }
                                    }
                                    .disabled(isReferenced)
                                }
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
                GroupBox("价格规则") {
                    VStack(spacing: 8) {
                        HStack {
                            Text("价格完全由用户配置，只用于成本估算。")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                            Spacer()
                            Button {
                                editingPolicy = PricingPolicy(
                                    id: "price-\(UUID().uuidString.prefix(6).lowercased())",
                                    version: "1",
                                    currency: "USD",
                                    rules: [PricingRule(
                                        modelPattern: "*",
                                        inputPerMillion: 0,
                                        outputPerMillion: 0
                                    )]
                                )
                            } label: { Image(systemName: "plus") }
                        }
                        ForEach(model.configuration.pricingPolicies) { policy in
                            HStack {
                                VStack(alignment: .leading) {
                                    Text(policy.id)
                                    Text("\(policy.currency) · v\(policy.version) · \(policy.rules.count) rules")
                                        .font(.caption2)
                                        .foregroundStyle(.secondary)
                                }
                                Spacer()
                                Button("编辑") { editingPolicy = policy }
                                Button(role: .destructive) {
                                    Task {
                                        var config = model.configuration
                                        config.pricingPolicies.removeAll { $0.id == policy.id }
                                        for index in config.routes.indices where config.routes[index].pricingPolicyID == policy.id {
                                            config.routes[index].pricingPolicyID = nil
                                        }
                                        _ = await model.applyConfiguration(config)
                                    }
                                } label: { Image(systemName: "trash") }
                            }
                        }
                    }
                    .padding(.vertical, 4)
                }
            }
            .padding(14)
        }
        .sheet(item: $editingPolicy) { policy in
            PricingPolicyEditor(policy: policy)
                .environmentObject(model)
        }
    }

}

private struct PricingPolicyEditor: View {
    @EnvironmentObject private var model: AppModel
    @Environment(\.dismiss) private var dismiss
    @State var policy: PricingPolicy
    private let originalID: String

    init(policy: PricingPolicy) {
        _policy = State(initialValue: policy)
        originalID = policy.id
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("价格规则").font(.title3.weight(.semibold))
            Form {
                TextField("ID", text: $policy.id)
                HStack {
                    TextField("版本", text: $policy.version)
                    TextField("货币", text: $policy.currency)
                }
                ForEach(policy.rules.indices, id: \.self) { index in
                    GroupBox("Rule \(index + 1)") {
                        VStack {
                            TextField("模型匹配", text: $policy.rules[index].modelPattern)
                            HStack {
                                decimalField("输入/百万", value: $policy.rules[index].inputPerMillion)
                                decimalField("缓存/百万", value: $policy.rules[index].cachedInputPerMillion)
                                decimalField("输出/百万", value: $policy.rules[index].outputPerMillion)
                            }
                        }
                    }
                }
                Button("添加 Rule") {
                    policy.rules.append(PricingRule(modelPattern: "*", inputPerMillion: 0, outputPerMillion: 0))
                }
            }
            HStack {
                Button("取消") { dismiss() }
                Spacer()
                Button("保存") {
                    Task {
                        var config = model.configuration
                        if let index = config.pricingPolicies.firstIndex(where: { $0.id == originalID }) {
                            config.pricingPolicies[index] = policy
                            if originalID != policy.id {
                                for routeIndex in config.routes.indices
                                where config.routes[routeIndex].pricingPolicyID == originalID {
                                    config.routes[routeIndex].pricingPolicyID = policy.id
                                }
                            }
                        } else {
                            config.pricingPolicies.append(policy)
                        }
                        if await model.applyConfiguration(config) { dismiss() }
                    }
                }
                .keyboardShortcut(.defaultAction)
            }
        }
        .padding(18)
        .frame(width: 520, height: 500)
    }

    private func decimalField(_ title: String, value: Binding<Decimal>) -> some View {
        TextField(title, value: value, format: .number)
    }
}

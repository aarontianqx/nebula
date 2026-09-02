import Foundation

public enum RequestDiagnosticPhase: String, Codable, Sendable, CaseIterable {
    case clientRequestStarted
    case clientRequestBodyActivity
    case clientRequestBodyCompleted
    case upstreamRequestStarted
    case upstreamResponseHeadReceived
    case upstreamBodyActivity
    case meaningfulSSEActivity
    case heartbeatActivity
    case downstreamWriteCompleted
    case upstreamCompleted

    public var displayName: String {
        switch self {
        case .clientRequestStarted: "客户端请求已开始"
        case .clientRequestBodyActivity: "正在接收客户端请求体"
        case .clientRequestBodyCompleted: "客户端请求体已完成"
        case .upstreamRequestStarted: "等待上游响应"
        case .upstreamResponseHeadReceived: "已收到上游响应头"
        case .upstreamBodyActivity: "上游响应传输中"
        case .meaningfulSSEActivity: "收到有效 SSE 数据"
        case .heartbeatActivity: "仅收到 SSE 心跳"
        case .downstreamWriteCompleted: "已写入客户端"
        case .upstreamCompleted: "上游响应已结束"
        }
    }
}

public enum RequestDiagnosticState: String, Codable, Sendable {
    case active
    case stalled
    case completed
    case failed
    case cancelled

    public var displayName: String {
        switch self {
        case .active: "进行中"
        case .stalled: "疑似卡住"
        case .completed: "已完成"
        case .failed: "失败"
        case .cancelled: "客户端取消"
        }
    }
}

public enum ResponseDiagnosticActivity: String, Codable, Sendable {
    case body
    case meaningfulSSE
    case heartbeat
    case sseTransport

    public var displayName: String {
        switch self {
        case .body: "响应数据"
        case .meaningfulSSE: "有效 SSE data"
        case .heartbeat: "SSE 心跳"
        case .sseTransport: "SSE 传输片段"
        }
    }
}

public struct RequestDiagnosticSnapshot: Codable, Identifiable, Equatable, Sendable {
    public let id: String
    public let routeID: String
    public let protocolType: UsageProtocol
    public let method: String
    public var phase: RequestDiagnosticPhase
    public var state: RequestDiagnosticState
    public let startedAt: Date
    public var updatedAt: Date
    public var lastUpstreamActivityAt: Date?
    public var lastMeaningfulActivityAt: Date
    public var lastDownstreamWriteAt: Date?
    public var lastResponseActivity: ResponseDiagnosticActivity?
    public var requestBytes: Int64
    public var responseBytes: Int64
    public var statusCode: Int?
    public var errorCode: String?

    public var idleDuration: TimeInterval {
        max(0, Date().timeIntervalSince(lastMeaningfulActivityAt))
    }
}

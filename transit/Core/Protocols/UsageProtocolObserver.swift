import Foundation
import NIOCore
import NIOFoundationCompat

public protocol UsageProtocolObserver: AnyObject {
    func observeRequest(contentType: String?, bytes: ByteBuffer)
    func observeResponse(contentType: String?, bytes: ByteBuffer)
    func finish(outcome: RequestOutcome) -> UsageObservation
}

public enum UsageObserverFactory {
    public static func make(
        protocolType: UsageProtocol,
        requestBufferLimit: Int = 1_048_576,
        responseBufferLimit: Int = 1_048_576,
        sseEventLimit: Int = 262_144
    ) -> UsageProtocolObserver {
        StreamingUsageObserver(
            protocolType: protocolType,
            requestBufferLimit: requestBufferLimit,
            responseBufferLimit: responseBufferLimit,
            sseEventLimit: sseEventLimit
        )
    }
}

private final class StreamingUsageObserver: UsageProtocolObserver {
    private let protocolType: UsageProtocol
    private let requestBufferLimit: Int
    private let responseBufferLimit: Int
    private let sseDecoder: SSEDecoder

    private var requestData = Data()
    private var responseData = Data()
    private var requestOverflow = false
    private var responseOverflow = false
    private var responseIsSSE = false
    private var accumulatedUsage: [String: Any]?
    private var chatRootUsageSeen = false
    private var responseModel: String?
    private var parserErrorCode: String?

    init(
        protocolType: UsageProtocol,
        requestBufferLimit: Int,
        responseBufferLimit: Int,
        sseEventLimit: Int
    ) {
        self.protocolType = protocolType
        self.requestBufferLimit = requestBufferLimit
        self.responseBufferLimit = responseBufferLimit
        sseDecoder = SSEDecoder(eventLimit: sseEventLimit)
    }

    func observeRequest(contentType: String?, bytes: ByteBuffer) {
        guard !requestOverflow, isJSON(contentType), let data = bytes.dataView else { return }
        if requestData.count + data.count > requestBufferLimit {
            requestData.removeAll(keepingCapacity: false)
            requestOverflow = true
        } else {
            requestData.append(data)
        }
    }

    func observeResponse(contentType: String?, bytes: ByteBuffer) {
        if contentType?.lowercased().contains("text/event-stream") == true {
            responseIsSSE = true
        }
        guard let data = bytes.dataView else { return }
        if responseIsSSE {
            let result = sseDecoder.feed(data)
            if result.overflowed { parserErrorCode = "sse_event_too_large" }
            for payload in result.payloads { observePayload(payload) }
        } else if !responseOverflow {
            if responseData.count + data.count > responseBufferLimit {
                responseData.removeAll(keepingCapacity: false)
                responseOverflow = true
                parserErrorCode = "response_too_large"
            } else {
                responseData.append(data)
            }
        }
    }

    func finish(outcome: RequestOutcome) -> UsageObservation {
        if responseIsSSE {
            let result = sseDecoder.finish()
            if result.overflowed { parserErrorCode = "sse_event_too_large" }
            for payload in result.payloads { observePayload(payload) }
        } else if !responseOverflow, !responseData.isEmpty {
            observePayload(responseData)
        }

        let requestModel = requestOverflow ? nil : JSONValue.object(from: requestData)?["model"].flatMap(JSONValue.string)
        guard let accumulatedUsage else {
            return UsageObservation(
                quality: .missing,
                model: responseModel ?? requestModel,
                endpointKind: endpointKind,
                parserErrorCode: parserErrorCode
            )
        }

        var normalized = normalize(accumulatedUsage)
        normalized.deriveTotalIfNeeded()
        guard normalized.hasAnyValue else {
            return UsageObservation(
                quality: .missing,
                model: responseModel ?? requestModel,
                endpointKind: endpointKind,
                parserErrorCode: parserErrorCode ?? "usage_without_numeric_fields"
            )
        }
        return UsageObservation(
            usage: normalized,
            quality: .reported,
            model: responseModel ?? requestModel,
            endpointKind: endpointKind,
            rawUsageJSON: JSONValue.canonicalString(accumulatedUsage),
            parserErrorCode: parserErrorCode
        )
    }

    private var endpointKind: EndpointKind {
        switch protocolType {
        case .openAIChat: .chat
        case .openAIResponses: .responses
        case .anthropicMessages: .messages
        }
    }

    private func observePayload(_ data: Data) {
        guard !data.isEmpty, data != Data("[DONE]".utf8) else { return }
        guard let object = JSONValue.object(from: data) else {
            parserErrorCode = "invalid_json_event"
            return
        }

        switch protocolType {
        case .openAIChat:
            responseModel = JSONValue.string(object["model"]) ?? responseModel
            // Chat-compatible providers put the final streaming usage either
            // at the response root or on a choice. Prefer the root form when
            // both are present; otherwise retain the first choice usage.
            if let usage = JSONValue.object(object["usage"]) {
                chatRootUsageSeen = true
                mergeUsage(usage)
            } else if !chatRootUsageSeen,
                      let usage = JSONValue.objects(object["choices"])
                .compactMap({ JSONValue.object($0["usage"]) })
                .first {
                mergeUsage(usage)
            }

        case .openAIResponses:
            responseModel = JSONValue.string(object["model"]) ?? responseModel
            if let usage = JSONValue.object(object["usage"]) {
                mergeUsage(usage)
            }
            if let response = JSONValue.object(object["response"]) {
                responseModel = JSONValue.string(response["model"]) ?? responseModel
                if let usage = JSONValue.object(response["usage"]) { mergeUsage(usage) }
            }

        case .anthropicMessages:
            responseModel = JSONValue.string(object["model"]) ?? responseModel
            if let usage = JSONValue.object(object["usage"]) { mergeUsage(usage) }
            if let message = JSONValue.object(object["message"]) {
                responseModel = JSONValue.string(message["model"]) ?? responseModel
                if let usage = JSONValue.object(message["usage"]) { mergeUsage(usage) }
            }
        }
    }

    private func mergeUsage(_ next: [String: Any]) {
        guard var current = accumulatedUsage else {
            accumulatedUsage = next
            return
        }
        current.merge(next) { existing, replacement in
            if let oldObject = JSONValue.object(existing), let newObject = JSONValue.object(replacement) {
                var merged = oldObject
                merged.merge(newObject) { _, new in new }
                return merged
            }
            return replacement
        }
        accumulatedUsage = current
    }

    private func normalize(_ usage: [String: Any]) -> NormalizedUsage {
        switch protocolType {
        case .openAIChat:
            let promptDetails = JSONValue.object(usage["prompt_tokens_details"])
            let completionDetails = JSONValue.object(usage["completion_tokens_details"])
            return NormalizedUsage(
                inputTokens: JSONValue.int64(usage["prompt_tokens"]),
                outputTokens: JSONValue.int64(usage["completion_tokens"]),
                cachedInputTokens: JSONValue.int64(usage["cached_tokens"])
                    ?? JSONValue.int64(promptDetails?["cached_tokens"]),
                reasoningTokens: JSONValue.int64(usage["reasoning_tokens"])
                    ?? JSONValue.int64(completionDetails?["reasoning_tokens"]),
                totalTokens: JSONValue.int64(usage["total_tokens"])
            )

        case .openAIResponses:
            let inputDetails = JSONValue.object(usage["input_tokens_details"])
            let outputDetails = JSONValue.object(usage["output_tokens_details"])
            return NormalizedUsage(
                inputTokens: JSONValue.int64(usage["input_tokens"]),
                outputTokens: JSONValue.int64(usage["output_tokens"]),
                cachedInputTokens: JSONValue.int64(inputDetails?["cached_tokens"]),
                reasoningTokens: JSONValue.int64(outputDetails?["reasoning_tokens"]),
                totalTokens: JSONValue.int64(usage["total_tokens"])
            )

        case .anthropicMessages:
            return NormalizedUsage(
                inputTokens: JSONValue.int64(usage["input_tokens"]),
                outputTokens: JSONValue.int64(usage["output_tokens"]),
                cachedInputTokens: JSONValue.int64(usage["cache_read_input_tokens"]),
                reasoningTokens: nil,
                totalTokens: JSONValue.int64(usage["total_tokens"])
            )
        }
    }

    private func isJSON(_ contentType: String?) -> Bool {
        guard let contentType else { return true }
        return contentType.lowercased().contains("json")
    }
}

private final class SSEDecoder {
    struct Result {
        var payloads: [Data] = []
        var overflowed = false
    }

    private let eventLimit: Int
    private var lineBuffer = Data()
    private var eventDataLines: [Data] = []
    private var eventSize = 0
    private var droppingCurrentEvent = false

    init(eventLimit: Int) {
        self.eventLimit = eventLimit
    }

    func feed(_ data: Data) -> Result {
        lineBuffer.append(data)
        var result = Result()
        while let newline = lineBuffer.firstIndex(of: 0x0A) {
            var line = Data(lineBuffer[..<newline])
            lineBuffer.removeSubrange(...newline)
            if line.last == 0x0D { line.removeLast() }
            consume(line: line, into: &result)
        }
        if lineBuffer.count > eventLimit {
            lineBuffer.removeAll(keepingCapacity: false)
            droppingCurrentEvent = true
            result.overflowed = true
        }
        return result
    }

    func finish() -> Result {
        var result = Result()
        if !lineBuffer.isEmpty {
            consume(line: lineBuffer, into: &result)
            lineBuffer.removeAll(keepingCapacity: false)
        }
        finishEvent(into: &result)
        return result
    }

    private func consume(line: Data, into result: inout Result) {
        if line.isEmpty {
            finishEvent(into: &result)
            return
        }
        guard !droppingCurrentEvent, line.starts(with: Data("data:".utf8)) else { return }
        var payload = line.dropFirst(5)
        if payload.first == 0x20 { payload = payload.dropFirst() }
        let data = Data(payload)
        eventSize += data.count
        if eventSize > eventLimit {
            droppingCurrentEvent = true
            eventDataLines.removeAll(keepingCapacity: false)
            result.overflowed = true
        } else {
            eventDataLines.append(data)
        }
    }

    private func finishEvent(into result: inout Result) {
        defer {
            eventDataLines.removeAll(keepingCapacity: true)
            eventSize = 0
            droppingCurrentEvent = false
        }
        guard !droppingCurrentEvent, !eventDataLines.isEmpty else { return }
        var joined = Data()
        for (index, line) in eventDataLines.enumerated() {
            if index > 0 { joined.append(0x0A) }
            joined.append(line)
        }
        result.payloads.append(joined)
    }
}

private enum JSONValue {
    static func object(from data: Data) -> [String: Any]? {
        guard !data.isEmpty,
              let value = try? JSONSerialization.jsonObject(with: data),
              let object = value as? [String: Any]
        else { return nil }
        return object
    }

    static func object(_ value: Any?) -> [String: Any]? {
        value as? [String: Any]
    }

    static func objects(_ value: Any?) -> [[String: Any]] {
        (value as? [Any])?.compactMap { $0 as? [String: Any] } ?? []
    }

    static func string(_ value: Any?) -> String? {
        value as? String
    }

    static func int64(_ value: Any?) -> Int64? {
        switch value {
        case let number as NSNumber:
            guard CFGetTypeID(number) != CFBooleanGetTypeID() else { return nil }
            return number.int64Value
        case let string as String:
            return Int64(string)
        default:
            return nil
        }
    }

    static func canonicalString(_ object: [String: Any]) -> String? {
        guard JSONSerialization.isValidJSONObject(object),
              let data = try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])
        else { return nil }
        return String(data: data, encoding: .utf8)
    }
}

private extension ByteBuffer {
    var dataView: Data? {
        getData(at: readerIndex, length: readableBytes)
    }
}

import NIOCore
import XCTest
@testable import TransitCore

final class UsageProtocolObserverTests: XCTestCase {
    func testOpenAIChatNonStreamingUsage() {
        let observer = UsageObserverFactory.make(protocolType: .openAIChat)
        observer.observeRequest(contentType: "application/json", bytes: buffer(#"{"model":"chat-model"}"#))
        observer.observeResponse(contentType: "application/json", bytes: buffer(#"""
        {
          "model":"resolved-chat-model",
          "usage":{
            "prompt_tokens":100,
            "completion_tokens":20,
            "total_tokens":120,
            "prompt_tokens_details":{"cached_tokens":60},
            "completion_tokens_details":{"reasoning_tokens":7}
          }
        }
        """#))

        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.model, "resolved-chat-model")
        XCTAssertEqual(result.endpointKind, .chat)
        XCTAssertEqual(result.usage.inputTokens, 100)
        XCTAssertEqual(result.usage.outputTokens, 20)
        XCTAssertEqual(result.usage.cachedInputTokens, 60)
        XCTAssertEqual(result.usage.reasoningTokens, 7)
        XCTAssertEqual(result.usage.totalTokens, 120)
        XCTAssertFalse(result.usage.totalTokensDerived)
    }

    func testOpenAIChatStreamingUsageOnChoice() {
        let observer = UsageObserverFactory.make(protocolType: .openAIChat)
        observer.observeRequest(contentType: "application/json", bytes: buffer(#"{"model":"chat-model","stream":true,"stream_options":{"include_usage":true}}"#))
        let first = #"data: {"choices":[{"index":0,"delta":{"content":"hello"}}]}"# + "\n\n"
        let final = #"data: {"choices":[{"index":0,"usage":{"prompt_tokens":120,"completion_tokens":30,"total_tokens":150,"prompt_tokens_details":{"cached_tokens":80},"reasoning_tokens":5}}]}"# + "\n\n"
        observer.observeResponse(contentType: "text/event-stream", bytes: buffer(first + final + "data: [DONE]\n\n"))

        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.usage.inputTokens, 120)
        XCTAssertEqual(result.usage.outputTokens, 30)
        XCTAssertEqual(result.usage.cachedInputTokens, 80)
        XCTAssertEqual(result.usage.reasoningTokens, 5)
        XCTAssertEqual(result.usage.totalTokens, 150)
        XCTAssertFalse(result.usage.totalTokensDerived)
    }

    func testOpenAIResponsesSSEAcrossChunks() {
        let observer = UsageObserverFactory.make(protocolType: .openAIResponses)
        let event = #"data: {"type":"response.completed","response":{"model":"response-model","usage":{"input_tokens":90,"output_tokens":10,"input_tokens_details":{"cached_tokens":50},"output_tokens_details":{"reasoning_tokens":4}}}}"# + "\n\n"
        let split = event.index(event.startIndex, offsetBy: 37)
        observer.observeResponse(contentType: "text/event-stream", bytes: buffer(String(event[..<split])))
        observer.observeResponse(contentType: "text/event-stream", bytes: buffer(String(event[split...])))

        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.model, "response-model")
        XCTAssertEqual(result.usage.inputTokens, 90)
        XCTAssertEqual(result.usage.outputTokens, 10)
        XCTAssertEqual(result.usage.cachedInputTokens, 50)
        XCTAssertEqual(result.usage.reasoningTokens, 4)
        XCTAssertEqual(result.usage.totalTokens, 100)
        XCTAssertTrue(result.usage.totalTokensDerived)
    }

    func testAnthropicStreamingUsageMergesEvents() {
        let observer = UsageObserverFactory.make(protocolType: .anthropicMessages)
        let start = #"data: {"type":"message_start","message":{"model":"messages-model","usage":{"input_tokens":200,"cache_read_input_tokens":150}}}"# + "\n\n"
        let delta = #"data: {"type":"message_delta","usage":{"output_tokens":"25"}}"# + "\n\n"
        observer.observeResponse(contentType: "text/event-stream", bytes: buffer(start + delta))

        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.model, "messages-model")
        XCTAssertEqual(result.usage.inputTokens, 200)
        XCTAssertEqual(result.usage.outputTokens, 25)
        XCTAssertEqual(result.usage.cachedInputTokens, 150)
        XCTAssertEqual(result.usage.totalTokens, 225)
    }

    func testMissingUsageDoesNotInventTokens() {
        let observer = UsageObserverFactory.make(protocolType: .openAIChat)
        observer.observeRequest(contentType: "application/json", bytes: buffer(#"{"model":"model-only"}"#))
        observer.observeResponse(contentType: "application/json", bytes: buffer(#"{"choices":[]}"#))
        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .missing)
        XCTAssertEqual(result.model, "model-only")
        XCTAssertFalse(result.usage.hasAnyValue)
    }

    func testOversizedSSEEventFailsObservationOnly() {
        let observer = UsageObserverFactory.make(protocolType: .openAIChat, sseEventLimit: 16)
        observer.observeResponse(
            contentType: "text/event-stream",
            bytes: buffer("data: {\"usage\":{\"prompt_tokens\":100}}\n\n")
        )
        let result = observer.finish(outcome: .completed)
        XCTAssertEqual(result.quality, .missing)
        XCTAssertEqual(result.parserErrorCode, "sse_event_too_large")
    }

    func testInvalidJSONEventIsDiagnosedWithoutHidingLaterUsage() {
        let observer = UsageObserverFactory.make(protocolType: .openAIResponses)
        let events = "data: {not-json}\n\n"
            + #"data: {"type":"response.completed","response":{"usage":{"input_tokens":8,"output_tokens":2}}}"#
            + "\n\n"
        observer.observeResponse(contentType: "text/event-stream", bytes: buffer(events))

        let result = observer.finish(outcome: .completed)

        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.usage.totalTokens, 10)
        XCTAssertEqual(result.parserErrorCode, "invalid_json_event")
    }

    func testRequestObservationOverflowDoesNotAffectResponseUsage() {
        let observer = UsageObserverFactory.make(protocolType: .openAIChat, requestBufferLimit: 8)
        observer.observeRequest(
            contentType: "application/json",
            bytes: buffer(#"{"model":"too-large-to-observe"}"#)
        )
        observer.observeResponse(
            contentType: "application/json",
            bytes: buffer(#"{"usage":{"prompt_tokens":5,"completion_tokens":1}}"#)
        )

        let result = observer.finish(outcome: .completed)

        XCTAssertNil(result.model)
        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.usage.totalTokens, 6)
    }

    func testCancelledObservationKeepsUsageAlreadyReceived() {
        let observer = UsageObserverFactory.make(protocolType: .anthropicMessages)
        observer.observeResponse(
            contentType: "text/event-stream",
            bytes: buffer(#"data: {"type":"message_start","message":{"usage":{"input_tokens":12}}}"# + "\n\n")
        )

        let result = observer.finish(outcome: .cancelled)

        XCTAssertEqual(result.quality, .reported)
        XCTAssertEqual(result.usage.inputTokens, 12)
        XCTAssertEqual(result.usage.totalTokens, 12)
    }

    private func buffer(_ string: String) -> ByteBuffer {
        var value = ByteBufferAllocator().buffer(capacity: string.utf8.count)
        value.writeString(string)
        return value
    }
}

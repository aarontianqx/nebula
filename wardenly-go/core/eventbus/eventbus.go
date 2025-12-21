// Package eventbus provides the event bus for publishing and subscribing to events.
package eventbus

import (
	"wardenly-go/core/event"
)

// EventBus is the interface for the event bus.
type EventBus interface {
	// Publish publishes an event to all subscribers.
	// This method is non-blocking; events are queued for async dispatch.
	Publish(e event.Event)

	// Subscribe subscribes to all events.
	// Returns a subscription ID that can be used to unsubscribe.
	Subscribe(handler EventHandler) string

	// SubscribeSession subscribes to events from a specific session.
	// Only events implementing SessionEvent with matching SessionID will be delivered.
	// Returns a subscription ID that can be used to unsubscribe.
	SubscribeSession(sessionID string, handler EventHandler) string

	// Unsubscribe removes a subscription by its ID.
	Unsubscribe(subscriptionID string)

	// Close shuts down the event bus and releases resources.
	// After Close is called, Publish will be a no-op.
	Close()
}

// EventHandler is a function that handles an event.
type EventHandler func(e event.Event)

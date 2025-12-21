package eventbus

import (
	"sync"
	"sync/atomic"

	"wardenly-go/core/event"
)

// subscription represents a single event subscription.
type subscription struct {
	id        string
	handler   EventHandler
	sessionID string // Empty string means subscribe to all events
}

// channelEventBus is a channel-based implementation of EventBus.
type channelEventBus struct {
	eventChan     chan event.Event
	subscriptions map[string]*subscription
	mu            sync.RWMutex
	closed        atomic.Bool
	wg            sync.WaitGroup
	nextID        atomic.Uint64
}

// New creates a new EventBus with the specified buffer size.
func New(bufferSize int) EventBus {
	if bufferSize <= 0 {
		bufferSize = 100
	}

	bus := &channelEventBus{
		eventChan:     make(chan event.Event, bufferSize),
		subscriptions: make(map[string]*subscription),
	}

	bus.wg.Add(1)
	go bus.dispatch()

	return bus
}

// Publish publishes an event to all subscribers.
func (b *channelEventBus) Publish(e event.Event) {
	if b.closed.Load() {
		return
	}

	// Non-blocking send with select to avoid blocking if buffer is full
	select {
	case b.eventChan <- e:
	default:
		// Buffer full, event dropped (could log this in production)
	}
}

// Subscribe subscribes to all events.
func (b *channelEventBus) Subscribe(handler EventHandler) string {
	return b.subscribe("", handler)
}

// SubscribeSession subscribes to events from a specific session.
func (b *channelEventBus) SubscribeSession(sessionID string, handler EventHandler) string {
	return b.subscribe(sessionID, handler)
}

func (b *channelEventBus) subscribe(sessionID string, handler EventHandler) string {
	id := b.generateID()

	b.mu.Lock()
	b.subscriptions[id] = &subscription{
		id:        id,
		handler:   handler,
		sessionID: sessionID,
	}
	b.mu.Unlock()

	return id
}

// Unsubscribe removes a subscription by its ID.
func (b *channelEventBus) Unsubscribe(subscriptionID string) {
	b.mu.Lock()
	delete(b.subscriptions, subscriptionID)
	b.mu.Unlock()
}

// Close shuts down the event bus.
func (b *channelEventBus) Close() {
	if b.closed.Swap(true) {
		return // Already closed
	}

	close(b.eventChan)
	b.wg.Wait()
}

// dispatch is the main event dispatch loop.
func (b *channelEventBus) dispatch() {
	defer b.wg.Done()

	for e := range b.eventChan {
		b.deliverEvent(e)
	}
}

// deliverEvent delivers an event to all matching subscribers.
func (b *channelEventBus) deliverEvent(e event.Event) {
	b.mu.RLock()
	// Copy subscriptions to avoid holding lock during handler execution
	subs := make([]*subscription, 0, len(b.subscriptions))
	for _, sub := range b.subscriptions {
		subs = append(subs, sub)
	}
	b.mu.RUnlock()

	// Get session ID if this is a session event
	var eventSessionID string
	if se, ok := e.(event.SessionEvent); ok {
		eventSessionID = se.SessionID()
	}

	for _, sub := range subs {
		// Filter by session ID if subscription is session-specific
		if sub.sessionID != "" {
			if eventSessionID == "" || sub.sessionID != eventSessionID {
				continue
			}
		}

		// Call handler (catch panics to prevent one bad handler from affecting others)
		func() {
			defer func() {
				if r := recover(); r != nil {
					// In production, log this panic
					_ = r
				}
			}()
			sub.handler(e)
		}()
	}
}

func (b *channelEventBus) generateID() string {
	id := b.nextID.Add(1)
	return string(rune('A'+id%26)) + string(rune('0'+id/26%10)) + string(rune('0'+id%10))
}

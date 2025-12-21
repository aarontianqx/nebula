package eventbus

import (
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"wardenly-go/core/event"
)

// mockEvent is a simple event for testing.
type mockEvent struct {
	name string
}

func (e *mockEvent) EventName() string {
	return e.name
}

// mockSessionEvent is a session event for testing.
type mockSessionEvent struct {
	name      string
	sessionID string
}

func (e *mockSessionEvent) EventName() string {
	return e.name
}

func (e *mockSessionEvent) SessionID() string {
	return e.sessionID
}

func TestEventBus_PublishSubscribe(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var received atomic.Int32
	var wg sync.WaitGroup
	wg.Add(1)

	bus.Subscribe(func(e event.Event) {
		received.Add(1)
		wg.Done()
	})

	bus.Publish(&mockEvent{name: "test"})

	// Wait for event to be delivered
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		if received.Load() != 1 {
			t.Errorf("Expected 1 event, got %d", received.Load())
		}
	case <-time.After(time.Second):
		t.Error("Timeout waiting for event")
	}
}

func TestEventBus_MultipleSubscribers(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var received atomic.Int32
	var wg sync.WaitGroup
	wg.Add(3) // 3 subscribers

	for i := 0; i < 3; i++ {
		bus.Subscribe(func(e event.Event) {
			received.Add(1)
			wg.Done()
		})
	}

	bus.Publish(&mockEvent{name: "test"})

	// Wait for all events to be delivered
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		if received.Load() != 3 {
			t.Errorf("Expected 3 events, got %d", received.Load())
		}
	case <-time.After(time.Second):
		t.Error("Timeout waiting for events")
	}
}

func TestEventBus_SessionFilter(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var session1Received atomic.Int32
	var session2Received atomic.Int32
	var allReceived atomic.Int32
	var wg sync.WaitGroup
	wg.Add(2) // session1 subscriber + all subscriber

	// Subscribe to session1 only
	bus.SubscribeSession("session1", func(e event.Event) {
		session1Received.Add(1)
		wg.Done()
	})

	// Subscribe to session2 only (should not receive)
	bus.SubscribeSession("session2", func(e event.Event) {
		session2Received.Add(1)
	})

	// Subscribe to all events
	bus.Subscribe(func(e event.Event) {
		allReceived.Add(1)
		wg.Done()
	})

	// Publish event for session1
	bus.Publish(&mockSessionEvent{name: "test", sessionID: "session1"})

	// Wait for events to be delivered
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		if session1Received.Load() != 1 {
			t.Errorf("session1 subscriber: expected 1, got %d", session1Received.Load())
		}
		if session2Received.Load() != 0 {
			t.Errorf("session2 subscriber: expected 0, got %d", session2Received.Load())
		}
		if allReceived.Load() != 1 {
			t.Errorf("all subscriber: expected 1, got %d", allReceived.Load())
		}
	case <-time.After(time.Second):
		t.Error("Timeout waiting for events")
	}
}

func TestEventBus_Unsubscribe(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var received atomic.Int32

	subID := bus.Subscribe(func(e event.Event) {
		received.Add(1)
	})

	// Unsubscribe
	bus.Unsubscribe(subID)

	// Publish event
	bus.Publish(&mockEvent{name: "test"})

	// Give some time for potential delivery
	time.Sleep(100 * time.Millisecond)

	if received.Load() != 0 {
		t.Errorf("Expected 0 events after unsubscribe, got %d", received.Load())
	}
}

func TestEventBus_Close(t *testing.T) {
	bus := New(10)

	var received atomic.Int32
	bus.Subscribe(func(e event.Event) {
		received.Add(1)
	})

	// Close the bus
	bus.Close()

	// Publish should be no-op after close
	bus.Publish(&mockEvent{name: "test"})

	// Give some time
	time.Sleep(100 * time.Millisecond)

	if received.Load() != 0 {
		t.Errorf("Expected 0 events after close, got %d", received.Load())
	}

	// Close again should not panic
	bus.Close()
}

func TestEventBus_HandlerPanic(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var received atomic.Int32
	var wg sync.WaitGroup
	wg.Add(1)

	// First handler panics
	bus.Subscribe(func(e event.Event) {
		panic("test panic")
	})

	// Second handler should still receive the event
	bus.Subscribe(func(e event.Event) {
		received.Add(1)
		wg.Done()
	})

	bus.Publish(&mockEvent{name: "test"})

	// Wait for event to be delivered
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		if received.Load() != 1 {
			t.Errorf("Expected 1 event despite panic, got %d", received.Load())
		}
	case <-time.After(time.Second):
		t.Error("Timeout waiting for event")
	}
}

func TestEventBus_NonSessionEventToSessionSubscriber(t *testing.T) {
	bus := New(10)
	defer bus.Close()

	var received atomic.Int32

	// Subscribe to session1 only
	bus.SubscribeSession("session1", func(e event.Event) {
		received.Add(1)
	})

	// Publish non-session event (should not be delivered to session subscriber)
	bus.Publish(&mockEvent{name: "test"})

	// Give some time
	time.Sleep(100 * time.Millisecond)

	if received.Load() != 0 {
		t.Errorf("Session subscriber should not receive non-session events, got %d", received.Load())
	}
}

func TestEventBus_ConcurrentPublish(t *testing.T) {
	bus := New(100)
	defer bus.Close()

	var received atomic.Int32
	var wg sync.WaitGroup

	const numEvents = 100
	wg.Add(numEvents)

	bus.Subscribe(func(e event.Event) {
		received.Add(1)
		wg.Done()
	})

	// Publish concurrently
	for i := 0; i < numEvents; i++ {
		go func(i int) {
			bus.Publish(&mockEvent{name: "test"})
		}(i)
	}

	// Wait for all events
	done := make(chan struct{})
	go func() {
		wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		if received.Load() != numEvents {
			t.Errorf("Expected %d events, got %d", numEvents, received.Load())
		}
	case <-time.After(5 * time.Second):
		t.Errorf("Timeout: received %d of %d events", received.Load(), numEvents)
	}
}

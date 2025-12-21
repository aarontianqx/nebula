package presentation

import (
	"image/color"
	"sync"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"
)

// SessionListItem represents a single item in the session list.
type SessionListItem struct {
	SessionID   string
	AccountName string
	IsRunning   bool
}

// SessionList is a scrollable list of sessions with status indicators.
type SessionList struct {
	widget.List
	items      []*SessionListItem
	itemsMu    sync.RWMutex
	onSelected func(sessionID string)
}

// NewSessionList creates a new session list widget.
func NewSessionList(onSelected func(sessionID string)) *SessionList {
	sl := &SessionList{
		items:      make([]*SessionListItem, 0),
		onSelected: onSelected,
	}

	sl.List = widget.List{
		Length: func() int {
			sl.itemsMu.RLock()
			defer sl.itemsMu.RUnlock()
			return len(sl.items)
		},
		CreateItem: func() fyne.CanvasObject {
			return sl.createItem()
		},
		UpdateItem: func(id widget.ListItemID, item fyne.CanvasObject) {
			sl.updateItem(id, item)
		},
	}

	sl.List.OnSelected = func(id widget.ListItemID) {
		sl.itemsMu.RLock()
		defer sl.itemsMu.RUnlock()

		if id >= 0 && id < len(sl.items) {
			if sl.onSelected != nil {
				sl.onSelected(sl.items[id].SessionID)
			}
		}
	}

	sl.ExtendBaseWidget(sl)
	return sl
}

func (sl *SessionList) createItem() fyne.CanvasObject {
	// Status indicator (circle) - slightly larger for better visibility
	indicator := canvas.NewCircle(color.RGBA{128, 128, 128, 255})
	indicator.Resize(fyne.NewSize(12, 12))

	// Account name label
	label := widget.NewLabel("Account Name")

	// Wrap in padded container for better touch targets and spacing
	row := container.NewHBox(
		container.NewCenter(container.NewGridWrap(fyne.NewSize(20, 20), indicator)),
		label,
	)

	return container.NewPadded(row)
}

func (sl *SessionList) updateItem(id widget.ListItemID, item fyne.CanvasObject) {
	sl.itemsMu.RLock()
	defer sl.itemsMu.RUnlock()

	if id >= len(sl.items) {
		return
	}

	data := sl.items[id]

	// Navigate through the padded container structure
	paddedContainer := item.(*fyne.Container)
	hbox := paddedContainer.Objects[0].(*fyne.Container)

	// Update indicator color
	indicatorContainer := hbox.Objects[0].(*fyne.Container)
	gridWrap := indicatorContainer.Objects[0].(*fyne.Container)
	indicator := gridWrap.Objects[0].(*canvas.Circle)

	if data.IsRunning {
		indicator.FillColor = color.RGBA{0, 200, 0, 255} // Green = running
	} else {
		indicator.FillColor = color.RGBA{128, 128, 128, 255} // Gray = idle
	}
	indicator.Refresh()

	// Update label
	label := hbox.Objects[1].(*widget.Label)
	label.SetText(data.AccountName)
}

// AddSession adds a new session to the list.
func (sl *SessionList) AddSession(sessionID, accountName string) {
	sl.itemsMu.Lock()
	sl.items = append(sl.items, &SessionListItem{
		SessionID:   sessionID,
		AccountName: accountName,
		IsRunning:   false,
	})
	sl.itemsMu.Unlock()

	sl.Refresh()
}

// RemoveSession removes a session from the list.
func (sl *SessionList) RemoveSession(sessionID string) {
	sl.itemsMu.Lock()
	for i, item := range sl.items {
		if item.SessionID == sessionID {
			sl.items = append(sl.items[:i], sl.items[i+1:]...)
			break
		}
	}
	sl.itemsMu.Unlock()

	sl.Refresh()
}

// UpdateSessionState updates the running state of a session.
func (sl *SessionList) UpdateSessionState(sessionID string, isRunning bool) {
	sl.itemsMu.Lock()
	for _, item := range sl.items {
		if item.SessionID == sessionID {
			item.IsRunning = isRunning
			break
		}
	}
	sl.itemsMu.Unlock()

	sl.Refresh()
}

// SelectSession programmatically selects a session by ID.
func (sl *SessionList) SelectSession(sessionID string) {
	sl.itemsMu.RLock()
	for i, item := range sl.items {
		if item.SessionID == sessionID {
			sl.itemsMu.RUnlock()
			// Unselect first to ensure OnSelected fires even if the same index was previously selected.
			// This handles the case where: open session -> close session -> open same session again.
			sl.UnselectAll()
			sl.Select(i)
			return
		}
	}
	sl.itemsMu.RUnlock()
}

// GetFirstSessionID returns the first session ID, or empty string if none.
func (sl *SessionList) GetFirstSessionID() string {
	sl.itemsMu.RLock()
	defer sl.itemsMu.RUnlock()

	if len(sl.items) > 0 {
		return sl.items[0].SessionID
	}
	return ""
}

// Count returns the number of sessions in the list.
func (sl *SessionList) Count() int {
	sl.itemsMu.RLock()
	defer sl.itemsMu.RUnlock()
	return len(sl.items)
}

// IndexOf returns the index of a session by ID, or -1 if not found.
func (sl *SessionList) IndexOf(sessionID string) int {
	sl.itemsMu.RLock()
	defer sl.itemsMu.RUnlock()
	for i, item := range sl.items {
		if item.SessionID == sessionID {
			return i
		}
	}
	return -1
}

// SessionIDAt returns the session ID at the given index, or empty string if out of bounds.
func (sl *SessionList) SessionIDAt(index int) string {
	sl.itemsMu.RLock()
	defer sl.itemsMu.RUnlock()
	if index < 0 || index >= len(sl.items) {
		return ""
	}
	return sl.items[index].SessionID
}

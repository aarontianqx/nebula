package presentation

import (
	"image"
	"log/slog"
	"sync"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/widget"
)

// CanvasWindow displays the browser view and handles user interactions.
type CanvasWindow struct {
	window    fyne.Window
	canvas    *BrowserCanvas
	isVisible bool
	logger    *slog.Logger
}

// NewCanvasWindow creates a new canvas window.
func NewCanvasWindow(app fyne.App) *CanvasWindow {
	w := &CanvasWindow{
		window:    app.NewWindow("Browser View"),
		canvas:    NewBrowserCanvas(fyne.NewSize(1080, 720)),
		isVisible: false,
		logger:    slog.Default(),
	}

	w.window.SetPadded(false)
	w.window.SetContent(w.canvas)
	w.window.Resize(fyne.NewSize(1080, 720))
	w.window.SetFixedSize(true)
	w.window.SetCloseIntercept(func() {
		// Do nothing, preventing window from closing
		// Window lifecycle is controlled by MainWindow
		w.logger.Info("Canvas window close intercepted")
	})

	return w
}

// Show displays the canvas window.
func (w *CanvasWindow) Show() {
	if !w.isVisible {
		w.window.Show()
		w.isVisible = true
	}
}

// Hide hides the canvas window.
func (w *CanvasWindow) Hide() {
	if w.isVisible {
		w.window.Hide()
		w.isVisible = false
	}
}

// Close closes the canvas window.
func (w *CanvasWindow) Close() {
	w.window.Close()
}

// SetOnClicked sets the click handler.
func (w *CanvasWindow) SetOnClicked(fn func(x, y float32)) {
	w.canvas.SetOnClicked(fn)
}

// SetOnDragged sets the drag handler.
func (w *CanvasWindow) SetOnDragged(fn func(fromX, fromY, toX, toY float32)) {
	w.canvas.SetOnDragged(fn)
}

// SetImage sets the displayed image.
func (w *CanvasWindow) SetImage(img image.Image) {
	if img == nil {
		return
	}
	w.canvas.SetImage(img)
}

// GetImage returns the current image.
func (w *CanvasWindow) GetImage() image.Image {
	return w.canvas.GetImage()
}

// IsVisible returns whether the window is visible.
func (w *CanvasWindow) IsVisible() bool {
	return w.isVisible
}

// ClearCallbacks clears all callbacks to avoid dangling references.
// This should be called when switching sessions or before hiding the window.
func (w *CanvasWindow) ClearCallbacks() {
	w.canvas.SetOnClicked(nil)
	w.canvas.SetOnDragged(nil)
}

// BrowserCanvas is a custom widget for displaying browser screenshots.
type BrowserCanvas struct {
	widget.BaseWidget
	canvas    *canvas.Image
	imageMu   sync.RWMutex
	onClicked func(x, y float32)
	onDragged func(fromX, fromY, toX, toY float32)
	dragMu    sync.Mutex
	dragRec   *dragRecord
}

type dragRecord struct {
	fromX, fromY, toX, toY float32
}

// NewBrowserCanvas creates a new browser canvas.
func NewBrowserCanvas(size fyne.Size) *BrowserCanvas {
	bc := &BrowserCanvas{
		canvas: canvas.NewImageFromImage(image.NewRGBA(image.Rect(0, 0, int(size.Width), int(size.Height)))),
	}
	bc.ExtendBaseWidget(bc)
	bc.canvas.Resize(size)
	bc.canvas.FillMode = canvas.ImageFillOriginal
	return bc
}

// SetImage sets the displayed image.
func (b *BrowserCanvas) SetImage(img image.Image) {
	if img == nil {
		return
	}
	b.imageMu.Lock()
	b.canvas.Image = img
	b.imageMu.Unlock()
	b.canvas.Refresh()
	b.Refresh()
}

// GetImage returns the current image.
func (b *BrowserCanvas) GetImage() image.Image {
	b.imageMu.RLock()
	defer b.imageMu.RUnlock()
	return b.canvas.Image
}

// CreateRenderer creates the widget renderer.
func (b *BrowserCanvas) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(b.canvas)
}

// SetOnClicked sets the click handler.
func (b *BrowserCanvas) SetOnClicked(fn func(x, y float32)) {
	b.onClicked = fn
}

// Tapped handles tap events.
func (b *BrowserCanvas) Tapped(e *fyne.PointEvent) {
	if b.onClicked != nil {
		b.onClicked(e.Position.X, e.Position.Y)
	}
}

// SetOnDragged sets the drag handler.
func (b *BrowserCanvas) SetOnDragged(fn func(fromX, fromY, toX, toY float32)) {
	b.onDragged = fn
}

// Dragged handles drag events.
func (b *BrowserCanvas) Dragged(e *fyne.DragEvent) {
	if b.onDragged != nil {
		b.dragMu.Lock()
		defer b.dragMu.Unlock()

		if b.dragRec == nil {
			b.dragRec = &dragRecord{
				fromX: e.AbsolutePosition.X - e.Dragged.DX,
				fromY: e.AbsolutePosition.Y - e.Dragged.DY,
				toX:   e.AbsolutePosition.X,
				toY:   e.AbsolutePosition.Y,
			}
		} else {
			b.dragRec.toX = e.AbsolutePosition.X
			b.dragRec.toY = e.AbsolutePosition.Y
		}
	}
}

// DragEnd handles drag end events.
func (b *BrowserCanvas) DragEnd() {
	if b.onDragged != nil {
		b.dragMu.Lock()
		defer b.dragMu.Unlock()

		if b.dragRec != nil {
			b.onDragged(b.dragRec.fromX, b.dragRec.fromY, b.dragRec.toX, b.dragRec.toY)
			b.dragRec = nil
		}
	}
}

// MinSize returns the minimum size of the canvas.
func (b *BrowserCanvas) MinSize() fyne.Size {
	return b.canvas.MinSize()
}

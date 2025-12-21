package ui

import (
	"image"
	_ "image/jpeg"
	_ "image/png"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/storage"
)

type MainWindow struct {
	window       fyne.Window
	controlPanel *ControlPanel
	canvasPanel  *CanvasPanel
}

func NewMainWindow(app fyne.App) *MainWindow {
	w := &MainWindow{
		window: app.NewWindow("Scene Analyzer"),
	}

	w.controlPanel = NewControlPanel()
	w.canvasPanel = NewCanvasPanel(w.controlPanel.HandleImageClick)

	// Create split container with control panel on left and canvas on right
	split := container.NewHSplit(
		w.controlPanel.Container(),
		w.canvasPanel,
	)
	split.SetOffset(0.3) // 30% width for control panel

	w.window.SetContent(split)
	w.window.Resize(fyne.NewSize(1200, 800))

	// Handle file drops
	w.window.SetOnDropped(func(pos fyne.Position, uris []fyne.URI) {
		if len(uris) == 0 {
			return
		}

		// Only handle the first dropped file
		uri := uris[0]
		reader, err := storage.Reader(uri)
		if err != nil {
			dialog.ShowError(err, w.window)
			return
		}
		defer reader.Close()

		// Try to decode as image
		if img, _, err := image.Decode(reader); err == nil {
			w.canvasPanel.LoadImage(img)
		} else {
			dialog.ShowError(err, w.window)
		}
	})

	return w
}

func (w *MainWindow) Show() {
	w.window.Show()
}

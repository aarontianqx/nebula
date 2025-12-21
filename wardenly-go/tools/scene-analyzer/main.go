package main

import (
	"scene-analyzer/internal/ui"

	"fyne.io/fyne/v2/app"
)

func main() {
	a := app.New()
	window := ui.NewMainWindow(a)
	window.Show()
	a.Run()
}

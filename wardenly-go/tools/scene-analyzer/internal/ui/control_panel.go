package ui

import (
	"fmt"
	"image"
	"image/color"
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/widget"
)

type ControlPanel struct {
	xEntry     *widget.Entry
	yEntry     *widget.Entry
	colorEntry *widget.Entry
	colorRect  *canvas.Rectangle
	container  *fyne.Container
	checkBtn   *widget.Button
	image      image.Image
}

func NewControlPanel() *ControlPanel {
	p := &ControlPanel{}

	// Coordinate inputs
	p.xEntry = widget.NewEntry()
	p.yEntry = widget.NewEntry()

	// Add Enter key handlers to coordinate inputs
	p.xEntry.OnSubmitted = func(string) { p.checkColorAtCurrentCoordinates() }
	p.yEntry.OnSubmitted = func(string) { p.checkColorAtCurrentCoordinates() }

	// Color display
	p.colorEntry = widget.NewEntry()
	p.colorEntry.Disable()
	p.colorEntry.TextStyle = fyne.TextStyle{Monospace: true}

	p.colorRect = canvas.NewRectangle(color.Black)
	p.colorRect.Resize(fyne.NewSize(40, 40))
	p.colorRect.SetMinSize(fyne.NewSize(40, 40))

	// Button to check color
	p.checkBtn = widget.NewButton("Check Color", func() {
		p.checkColorAtCurrentCoordinates()
	})

	// Layout construction
	coordBox := container.New(layout.NewHBoxLayout(),
		widget.NewLabel("X:"),
		container.NewGridWrap(fyne.NewSize(50, 40), p.xEntry),
		widget.NewLabel("Y:"),
		container.NewGridWrap(fyne.NewSize(50, 40), p.yEntry),
	)

	colorBox := container.New(layout.NewHBoxLayout(),
		widget.NewLabel("Color:"),
		container.NewGridWrap(fyne.NewSize(150, 40), p.colorEntry),
		container.NewGridWrap(fyne.NewSize(40, 40), p.colorRect),
	)

	p.container = container.NewVBox(
		coordBox,
		p.checkBtn,
		colorBox,
	)

	return p
}

func (p *ControlPanel) Container() fyne.CanvasObject {
	return p.container
}

func (p *ControlPanel) HandleImageClick(x, y int, img image.Image) {
	p.image = img
	p.xEntry.SetText(fmt.Sprintf("%d", x))
	p.yEntry.SetText(fmt.Sprintf("%d", y))
	p.updateColorDisplay(x, y)
}

func (p *ControlPanel) updateColorDisplay(x, y int) {
	if p.image == nil {
		return
	}
	c := p.image.At(x, y)
	r, g, b, _ := c.RGBA()
	r, g, b = r>>8, g>>8, b>>8
	p.colorEntry.SetText(fmt.Sprintf("RGB(%d, %d, %d)", r, g, b))
	p.colorRect.FillColor = c
	p.colorRect.Refresh()
}

func (p *ControlPanel) GetCoordinates() (x, y int, err error) {
	x, err = strconv.Atoi(p.xEntry.Text)
	if err != nil {
		return 0, 0, fmt.Errorf("invalid X coordinate: %v", err)
	}

	y, err = strconv.Atoi(p.yEntry.Text)
	if err != nil {
		return 0, 0, fmt.Errorf("invalid Y coordinate: %v", err)
	}

	return x, y, nil
}

func (p *ControlPanel) checkColorAtCurrentCoordinates() {
	x, errX := strconv.Atoi(p.xEntry.Text)
	y, errY := strconv.Atoi(p.yEntry.Text)
	if errX == nil && errY == nil && p.image != nil {
		p.updateColorDisplay(x, y)
	}
}

package ui

import (
	"image"
	"image/color"
	_ "image/jpeg"
	_ "image/png"
	"log/slog"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/widget"
)

type CanvasPanel struct {
	widget.BaseWidget
	image     *canvas.Image
	onClicked func(x, y int, img image.Image)
	container *fyne.Container
}

func NewCanvasPanel(onClick func(x, y int, img image.Image)) *CanvasPanel {
	p := &CanvasPanel{
		onClicked: onClick,
	}

	p.image = canvas.NewImageFromImage(nil)
	p.image.FillMode = canvas.ImageFillOriginal

	p.container = container.NewWithoutLayout(p.image)

	p.ExtendBaseWidget(p)
	return p
}

func (p *CanvasPanel) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(p.container)
}

func (p *CanvasPanel) Tapped(e *fyne.PointEvent) {
	if p.image.Image == nil {
		return
	}

	x := int(e.Position.X)
	y := int(e.Position.Y)

	bounds := p.image.Image.Bounds()
	if x >= bounds.Min.X && x < bounds.Max.X &&
		y >= bounds.Min.Y && y < bounds.Max.Y {
		if p.onClicked != nil {
			p.onClicked(x, y, p.image.Image)
		}
	}
}

func (p *CanvasPanel) LoadImage(img image.Image) {
	if img == nil {
		return
	}
	p.image.Image = img
	p.image.Refresh()
	bounds := img.Bounds()
	slog.Info("image loaded", "bound", img.Bounds(),
		"color", img.At(62, 642).(color.RGBA),
		"color", img.At(674, 15).(color.RGBA),
	)
	p.image.Resize(fyne.NewSize(
		float32(bounds.Max.X-bounds.Min.X),
		float32(bounds.Max.Y-bounds.Min.Y),
	))
	p.container.Resize(p.image.Size())
	p.Refresh()
}

func (p *CanvasPanel) MinSize() fyne.Size {
	return p.image.Size()
}

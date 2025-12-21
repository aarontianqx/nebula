# Scene Analyzer

A simple image analysis tool built with Fyne that allows you to inspect pixel colors at specific coordinates in an image.

## Features

- Drag and drop image files to load them into the canvas
- Click on any point in the image to get its RGB color value
- Manually enter X,Y coordinates and press Enter to get the color at that position
- Visual display of the selected color
- Compatible with high DPI displays (4K)

## Usage

1. Run the application
2. Drag and drop an image file into the window
3. Either:
   - Click directly on a pixel in the image to analyze its color
   - Enter X,Y coordinates in the input fields and press Enter

The color information will be displayed in RGB format along with a color sample.

## Building

```
cd tools/scene-analyzer
go build
```

## Dependencies

- [Fyne](https://fyne.io/) - Cross-platform GUI toolkit 
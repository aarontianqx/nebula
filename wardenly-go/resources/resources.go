package resources

import (
	"embed"
	"fyne.io/fyne/v2"
)

//go:embed icons/app_256.png
var iconData []byte

func GetAppIcon() fyne.Resource {
	return &fyne.StaticResource{
		StaticName:    "app_256.png",
		StaticContent: iconData,
	}
}

//go:embed scripts/*.yaml
var ScriptFiles embed.FS

//go:embed scenes/*.yaml
var SceneFiles embed.FS

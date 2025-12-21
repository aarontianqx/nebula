// Package main is the entry point for Wardenly.
package main

import (
	"context"
	"os"
	"time"

	"wardenly-go/application"
	"wardenly-go/core/eventbus"
	domainaccount "wardenly-go/domain/account"
	domaingroup "wardenly-go/domain/group"
	domainscene "wardenly-go/domain/scene"
	domainscript "wardenly-go/domain/script"
	"wardenly-go/infrastructure/browser"
	"wardenly-go/infrastructure/logging"
	"wardenly-go/infrastructure/ocr"
	"wardenly-go/infrastructure/repository"
	"wardenly-go/presentation"
	"wardenly-go/resources"

	"fyne.io/fyne/v2/app"
)

func main() {
	// Initialize logging (dev: console only, prod: rotating file)
	logger, closeLog, err := logging.Setup(nil)
	if err != nil {
		// Fallback to stderr if logging setup fails
		os.Stderr.WriteString("Failed to initialize logging: " + err.Error() + "\n")
		os.Exit(1)
	}
	defer closeLog()

	logger.Info("Starting Wardenly")

	ctx := context.Background()

	// Initialize MongoDB
	mongoDB, err := repository.NewMongoDB(ctx, repository.DefaultMongoDBConfig(), logger)
	if err != nil {
		logger.Error("Failed to initialize MongoDB", "error", err)
		os.Exit(1)
	}
	defer mongoDB.Close(ctx)

	// Initialize repositories
	accountRepo := repository.NewMongoAccountRepository(mongoDB, logger)
	groupRepo := repository.NewMongoGroupRepository(mongoDB, logger)

	// Initialize domain services
	accountService := domainaccount.NewService(accountRepo)
	groupService := domaingroup.NewService(groupRepo, accountRepo)

	// Initialize OCR client
	ocrConfig := ocr.DefaultClientConfig()
	ocrClient := ocr.NewHTTPClient(ocrConfig)
	defer ocrClient.Close()

	// Load scenes
	sceneRegistry := domainscene.NewRegistry()
	sceneLoader := domainscene.NewLoader(sceneRegistry)
	if err := sceneLoader.LoadFromFS(resources.SceneFiles); err != nil {
		logger.Error("Failed to load scenes", "error", err)
		os.Exit(1)
	}
	logger.Info("Scenes loaded", "count", sceneRegistry.Count())

	// Load scripts
	scriptRegistry := domainscript.NewRegistry()
	scriptLoader := domainscript.NewLoader(scriptRegistry)
	if err := scriptLoader.LoadFromFS(resources.ScriptFiles); err != nil {
		logger.Error("Failed to load scripts", "error", err)
		os.Exit(1)
	}
	logger.Info("Scripts loaded", "count", scriptRegistry.Count())

	// Initialize event bus
	eventBus := eventbus.New(100)
	defer eventBus.Close()

	// Initialize coordinator
	coordinator := application.NewCoordinator(&application.CoordinatorConfig{
		EventBus:       eventBus,
		SceneRegistry:  sceneRegistry,
		ScriptRegistry: scriptRegistry,
		OCRClient:      ocrClient,
		DriverFactory: func() browser.Driver {
			// Use default config which has Headless=true
			// Browser runs headless, screenshots are captured via chromedp and displayed in CanvasWindow
			return browser.NewChromeDPDriver(browser.DefaultDriverConfig())
		},
		Logger: logger,
	})
	coordinator.Start()
	defer coordinator.Stop()

	// Initialize UI event bridge
	bridge := presentation.NewUIEventBridge(&presentation.BridgeConfig{
		Coordinator: coordinator,
		EventBus:    eventBus,
		Logger:      logger,
	})
	defer bridge.Close()

	// Initialize Fyne app
	fyneApp := app.New()
	fyneApp.SetIcon(resources.GetAppIcon())

	// Get script names for UI
	scriptNames := scriptRegistry.List()

	// Initialize main window
	mainWindow := presentation.NewMainWindow(&presentation.MainWindowConfig{
		App:            fyneApp,
		Bridge:         bridge,
		Logger:         logger,
		AccountService: accountService,
		GroupService:   groupService,
		ScriptNames:    scriptNames,
	})
	defer mainWindow.Cleanup()

	// Show and run
	mainWindow.Show()
	fyneApp.Run()

	// Start shutdown timeout - force exit after 10 seconds if cleanup hangs
	go func() {
		time.Sleep(10 * time.Second)
		logger.Warn("Shutdown timeout, forcing exit")
		os.Exit(0)
	}()

	logger.Info("Application shutdown complete")
}

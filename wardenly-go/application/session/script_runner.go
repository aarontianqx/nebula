package session

import (
	"context"
	"fmt"
	"image"
	"log/slog"
	"sync"
	"sync/atomic"
	"time"

	"wardenly-go/core/event"
	domainscript "wardenly-go/domain/script"
	"wardenly-go/infrastructure/ocr"
)

// ScriptRunner executes automation scripts for a session.
type ScriptRunner struct {
	session *Session
	logger  *slog.Logger

	// Execution state
	running   atomic.Bool
	script    *domainscript.Script
	counters  map[string]int
	counterMu sync.Mutex

	// Control
	ctx    context.Context
	cancel context.CancelFunc
	wg     sync.WaitGroup
}

// NewScriptRunner creates a new script runner.
func NewScriptRunner(session *Session, logger *slog.Logger) *ScriptRunner {
	if logger == nil {
		logger = slog.Default()
	}
	return &ScriptRunner{
		session:  session,
		logger:   logger,
		counters: make(map[string]int),
	}
}

// Start begins executing the specified script.
func (r *ScriptRunner) Start(script *domainscript.Script) {
	if r.running.Load() {
		r.logger.Warn("Script already running")
		return
	}

	r.script = script
	r.counters = make(map[string]int)
	r.running.Store(true)
	r.ctx, r.cancel = context.WithCancel(r.session.Context())

	r.wg.Add(1)
	go r.run()

	r.logger.Info("Script started", "name", script.Name)
}

// Stop signals the script to stop.
func (r *ScriptRunner) Stop() {
	if !r.running.Load() {
		return
	}
	r.running.Store(false)
	if r.cancel != nil {
		r.cancel()
	}

	done := make(chan struct{})
	go func() {
		r.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		r.logger.Warn("Script stop timeout")
	}
	r.logger.Info("Script stopped")
}

// IsRunning returns true if a script is currently running.
func (r *ScriptRunner) IsRunning() bool {
	return r.running.Load()
}

// run is the main script execution loop.
func (r *ScriptRunner) run() {
	defer r.wg.Done()
	defer r.cleanup()

	scriptName := r.script.Name
	var stopReason event.StopReason
	var stopErr error

	defer func() {
		if rec := recover(); rec != nil {
			r.logger.Error("Script execution panicked", "error", rec)
			stopReason = event.StopReasonError
			stopErr = fmt.Errorf("panic: %v", rec)
		}
		r.session.OnScriptStopped(scriptName, stopReason, stopErr)
	}()

	const defaultWaitDuration = 500 * time.Millisecond

	for r.running.Load() {
		select {
		case <-r.ctx.Done():
			stopReason = event.StopReasonManual
			return
		default:
		}

		// Check if browser is still running
		if !r.session.GetBrowserController().IsRunning() {
			r.logger.Info("Browser stopped, ending script")
			stopReason = event.StopReasonBrowserStopped
			return
		}

		// Capture current screen
		screen, err := r.session.GetScreenCapture().Capture(r.ctx)
		if err != nil {
			r.logger.Warn("Failed to capture screen", "error", err)
			if r.running.Load() {
				time.Sleep(defaultWaitDuration)
			}
			continue
		}

		// Try to find matching scene
		var matchedStep *domainscript.Step
		for i := range r.script.Steps {
			step := &r.script.Steps[i]
			scene := r.session.GetSceneRegistry().FindMatch(
				screen,
				r.session.GetSceneMatcher(),
				step.ExpectedScene,
			)
			if scene != nil {
				matchedStep = step
				break
			}
		}

		if matchedStep == nil {
			time.Sleep(defaultWaitDuration)
			continue
		}

		// Execute matched step
		result := r.executeStep(matchedStep, screen)
		if result == stepResultQuit {
			stopReason = event.StopReasonNormal
			return
		}
		if result == stepResultResourceExhausted {
			stopReason = event.StopReasonResourceExhausted
			return
		}

		if r.running.Load() {
			time.Sleep(defaultWaitDuration)
		}
	}

	stopReason = event.StopReasonManual
}

func (r *ScriptRunner) cleanup() {
	r.running.Store(false)
	r.counters = make(map[string]int)
}

type stepResult int

const (
	stepResultContinue stepResult = iota
	stepResultQuit
	stepResultResourceExhausted
	stepResultError
)

// executeStep executes a single script step.
func (r *ScriptRunner) executeStep(step *domainscript.Step, screen image.Image) stepResult {
	if !r.running.Load() {
		return stepResultQuit
	}

	// Check OCR rule before executing actions
	if step.OCRRule != nil {
		shouldStop, err := r.checkOCRRule(step.ExpectedScene, step.OCRRule, screen)
		if err != nil {
			r.logger.Error("OCR rule check failed", "error", err)
			return stepResultQuit
		}
		if shouldStop {
			r.logger.Info("OCR rule triggered, stopping script", "rule", step.OCRRule.Name)
			return stepResultResourceExhausted
		}
	}

	// Execute actions
	if step.Loop == nil {
		return r.executeActions(step.Actions, step)
	}

	// Handle looped actions
	return r.executeLoopedStep(step)
}

// executeLoopedStep handles step execution with loop.
func (r *ScriptRunner) executeLoopedStep(step *domainscript.Step) stepResult {
	loop := step.Loop

	// Validate loop indices
	if err := loop.ValidateIndices(len(step.Actions)); err != nil {
		r.logger.Error("Invalid loop configuration", "error", err)
		return stepResultError
	}

	startIdx := loop.StartIndex
	endIdx := loop.EndIndex

	// Execute pre-loop actions
	if startIdx > 0 {
		if result := r.executeActions(step.Actions[:startIdx], step); result != stepResultContinue {
			return result
		}
	}

	// Execute loop
	iteration := 0
	for r.running.Load() {
		select {
		case <-r.ctx.Done():
			return stepResultQuit
		default:
		}

		// Execute loop actions
		if result := r.executeActions(step.Actions[startIdx:endIdx+1], step); result != stepResultContinue {
			return result
		}

		// Check until condition
		if loop.HasUntilCondition() {
			screen, err := r.session.GetScreenCapture().Capture(r.ctx)
			if err != nil {
				r.logger.Warn("Failed to capture screen in loop", "error", err)
				break
			}
			scene := r.session.GetSceneRegistry().FindMatch(
				screen,
				r.session.GetSceneMatcher(),
				loop.Until,
			)
			if scene != nil {
				break
			}
		}

		iteration++
		if loop.Count > 0 && iteration >= loop.Count {
			break
		}

		if loop.Interval > 0 {
			select {
			case <-r.ctx.Done():
				return stepResultQuit
			case <-time.After(loop.Interval):
			}
		}
	}

	// Execute post-loop actions
	if endIdx+1 < len(step.Actions) {
		return r.executeActions(step.Actions[endIdx+1:], step)
	}

	return stepResultContinue
}

// executeActions executes a list of actions.
func (r *ScriptRunner) executeActions(actions []domainscript.Action, step *domainscript.Step) stepResult {
	for _, action := range actions {
		if !r.running.Load() {
			return stepResultQuit
		}

		select {
		case <-r.ctx.Done():
			return stepResultQuit
		default:
		}

		result := r.executeAction(&action, step)
		if result != stepResultContinue {
			return result
		}
	}
	return stepResultContinue
}

// executeAction executes a single action.
func (r *ScriptRunner) executeAction(action *domainscript.Action, step *domainscript.Step) stepResult {
	ctx := r.ctx
	browserCtrl := r.session.GetBrowserController()

	switch action.Type {
	case domainscript.ActionTypeClick:
		if len(action.Points) < 1 {
			r.logger.Error("Click action requires a point")
			return stepResultError
		}
		if err := browserCtrl.Click(ctx, action.Points[0].X, action.Points[0].Y); err != nil {
			r.logger.Error("Click failed", "error", err)
			return stepResultError
		}

	case domainscript.ActionTypeWait:
		select {
		case <-ctx.Done():
			return stepResultQuit
		case <-time.After(action.Duration):
		}

	case domainscript.ActionTypeDrag:
		if len(action.Points) < 2 {
			r.logger.Error("Drag action requires at least 2 points")
			return stepResultError
		}
		// Convert points
		points := make([]struct{ X, Y float64 }, len(action.Points))
		for i, p := range action.Points {
			points[i] = struct{ X, Y float64 }{p.X, p.Y}
		}
		if err := browserCtrl.Drag(ctx, points[0].X, points[0].Y, points[len(points)-1].X, points[len(points)-1].Y); err != nil {
			r.logger.Error("Drag failed", "error", err)
			return stepResultError
		}

	case domainscript.ActionTypeIncr:
		if action.Key == "" {
			r.logger.Error("Incr action requires a key")
			return stepResultError
		}
		r.counterMu.Lock()
		r.counters[action.Key]++
		r.counterMu.Unlock()

	case domainscript.ActionTypeDecr:
		if action.Key == "" {
			r.logger.Error("Decr action requires a key")
			return stepResultError
		}
		r.counterMu.Lock()
		r.counters[action.Key]--
		r.counterMu.Unlock()

	case domainscript.ActionTypeQuit:
		if action.Condition != nil {
			r.counterMu.Lock()
			shouldQuit := action.Condition.Evaluate(r.counters)
			r.counterMu.Unlock()
			if shouldQuit {
				return stepResultQuit
			}
		} else {
			return stepResultQuit
		}

	case domainscript.ActionTypeCheckScene:
		// Check OCR rule if defined
		if step != nil && step.OCRRule != nil {
			screen, err := r.session.GetScreenCapture().Capture(ctx)
			if err != nil {
				r.logger.Warn("Failed to capture screen for check_scene", "error", err)
				return stepResultContinue
			}
			shouldStop, err := r.checkOCRRule(step.ExpectedScene, step.OCRRule, screen)
			if err != nil {
				r.logger.Error("OCR rule check failed in check_scene", "error", err)
				return stepResultQuit
			}
			if shouldStop {
				r.logger.Info("OCR rule triggered in check_scene", "rule", step.OCRRule.Name)
				return stepResultResourceExhausted
			}
		}

	default:
		r.logger.Warn("Unknown action type", "type", action.Type)
	}

	return stepResultContinue
}

// checkOCRRule checks if an OCR rule condition is met.
func (r *ScriptRunner) checkOCRRule(expectedScene string, rule *domainscript.OCRRule, screen image.Image) (bool, error) {
	if rule == nil {
		return false, nil
	}

	// Validate rule name
	switch rule.Name {
	case "quit_when_exhausted":
		// Valid rule
	default:
		return false, fmt.Errorf("unknown OCR rule: %s", rule.Name)
	}

	// Check if scene matches
	scene := r.session.GetSceneRegistry().FindMatch(
		screen,
		r.session.GetSceneMatcher(),
		expectedScene,
	)
	if scene == nil {
		return false, nil // Scene doesn't match, skip OCR check
	}

	// Get OCR client
	ocrClient := r.session.GetOCRClient()
	if ocrClient == nil || !ocrClient.IsHealthy() {
		r.logger.Warn("OCR client not available, skipping rule check")
		return false, nil
	}

	// Perform OCR
	roi := &ocr.ROI{
		X:      rule.ROI.X,
		Y:      rule.ROI.Y,
		Width:  rule.ROI.Width,
		Height: rule.ROI.Height,
	}

	result, err := ocrClient.RecognizeUsageRatioFromImage(r.ctx, screen, roi)
	if err != nil {
		r.logger.Warn("OCR recognition failed", "error", err)
		return false, nil // Don't stop on OCR failure
	}

	r.logger.Info("OCR result",
		"rule", rule.Name,
		"numerator", result.Numerator,
		"denominator", result.Denominator,
		"threshold", rule.Threshold,
	)

	// Check quit_when_exhausted rule
	if rule.Name == "quit_when_exhausted" {
		if result.Denominator > rule.Threshold || result.Denominator > result.Numerator {
			return true, nil
		}
	}

	return false, nil
}

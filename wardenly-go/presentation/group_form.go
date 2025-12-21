package presentation

import (
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"wardenly-go/domain/account"
	"wardenly-go/domain/group"
)

// GroupFormConfig holds configuration for GroupForm.
type GroupFormConfig struct {
	OnSave   func(*group.Group)
	OnDelete func(*group.Group)
}

// GroupForm provides a form for editing group details.
type GroupForm struct {
	config    *GroupFormConfig
	container *fyne.Container

	// Form fields
	nameEntry        *widget.Entry
	descriptionEntry *widget.Entry
	rankingEntry     *widget.Entry

	// Member selection
	memberChecks   []*widget.Check
	memberPanel    *fyne.Container
	memberScroll   *container.Scroll
	selectAllBtn   *widget.Button
	deselectAllBtn *widget.Button
	allAccounts    []*account.Account

	// Buttons
	saveBtn   *widget.Button
	deleteBtn *widget.Button

	// Current group being edited
	current *group.Group
}

// NewGroupForm creates a new group editing form.
func NewGroupForm(cfg *GroupFormConfig) *GroupForm {
	gf := &GroupForm{config: cfg}
	gf.build()
	return gf
}

func (gf *GroupForm) build() {
	gf.nameEntry = widget.NewEntry()
	gf.nameEntry.SetPlaceHolder("Group name")

	gf.descriptionEntry = widget.NewMultiLineEntry()
	gf.descriptionEntry.SetPlaceHolder("Optional description")
	gf.descriptionEntry.SetMinRowsVisible(2)

	gf.rankingEntry = widget.NewEntry()
	gf.rankingEntry.SetPlaceHolder("Sort priority (lower = higher)")

	// Use widget.Form for proper alignment
	form := widget.NewForm(
		widget.NewFormItem("Name", gf.nameEntry),
		widget.NewFormItem("Description", gf.descriptionEntry),
		widget.NewFormItem("Ranking", gf.rankingEntry),
	)

	// Member selection with Select All / Deselect All buttons
	gf.selectAllBtn = widget.NewButton("Select All", gf.onSelectAll)
	gf.deselectAllBtn = widget.NewButton("Deselect All", gf.onDeselectAll)
	memberToolbar := container.NewHBox(gf.selectAllBtn, gf.deselectAllBtn)

	gf.memberPanel = container.NewVBox()
	gf.memberScroll = container.NewVScroll(gf.memberPanel)
	// No SetMinSize - let BorderLayout handle sizing

	// Member header with label
	memberHeader := widget.NewLabelWithStyle("Members", fyne.TextAlignLeading, fyne.TextStyle{Bold: true})

	// Buttons with icons - Delete on left, Save on right
	gf.deleteBtn = widget.NewButtonWithIcon("Delete", theme.DeleteIcon(), gf.onDelete)
	gf.deleteBtn.Importance = widget.DangerImportance

	gf.saveBtn = widget.NewButtonWithIcon("Save", theme.DocumentSaveIcon(), gf.onSave)
	gf.saveBtn.Importance = widget.HighImportance

	buttonBar := container.NewHBox(
		gf.deleteBtn,
		layout.NewSpacer(),
		gf.saveBtn,
	)

	// Top section: form + member header + toolbar
	topSection := container.NewVBox(
		form,
		widget.NewSeparator(),
		memberHeader,
		memberToolbar,
	)

	// Bottom section: separator + buttons
	bottomSection := container.NewVBox(
		widget.NewSeparator(),
		buttonBar,
	)

	// Use BorderLayout: members list fills remaining vertical space
	gf.container = container.NewPadded(container.NewBorder(
		topSection,    // Top
		bottomSection, // Bottom
		nil, nil,
		gf.memberScroll, // Center - fills all remaining space
	))
}

func (gf *GroupForm) onSelectAll() {
	for _, check := range gf.memberChecks {
		check.SetChecked(true)
	}
}

func (gf *GroupForm) onDeselectAll() {
	for _, check := range gf.memberChecks {
		check.SetChecked(false)
	}
}

// Container returns the form container.
func (gf *GroupForm) Container() fyne.CanvasObject {
	return gf.container
}

// SetGroup populates the form with group data.
// Pass nil to clear the form for creating a new group.
// accounts is the list of all available accounts for member selection.
func (gf *GroupForm) SetGroup(grp *group.Group, accounts []*account.Account) {
	gf.current = grp
	gf.allAccounts = accounts

	// Rebuild member checkboxes
	gf.memberChecks = make([]*widget.Check, len(accounts))
	gf.memberPanel.Objects = nil

	for i, acc := range accounts {
		check := widget.NewCheck(acc.Identity(), nil)
		gf.memberChecks[i] = check
		gf.memberPanel.Add(check)
	}

	if grp == nil {
		gf.nameEntry.SetText("")
		gf.descriptionEntry.SetText("")
		gf.rankingEntry.SetText("0")
		gf.deleteBtn.Disable()
		// Uncheck all
		for _, check := range gf.memberChecks {
			check.SetChecked(false)
		}
	} else {
		gf.nameEntry.SetText(grp.Name)
		gf.descriptionEntry.SetText(grp.Description)
		gf.rankingEntry.SetText(strconv.Itoa(grp.Ranking))
		gf.deleteBtn.Enable()
		// Check members that are in the group
		memberSet := make(map[string]bool)
		for _, id := range grp.AccountIDs {
			memberSet[id] = true
		}
		for i, acc := range accounts {
			gf.memberChecks[i].SetChecked(memberSet[acc.ID])
		}
	}

	gf.memberPanel.Refresh()
}

// Clear resets the form to empty state.
func (gf *GroupForm) Clear() {
	gf.SetGroup(nil, gf.allAccounts)
}

func (gf *GroupForm) onSave() {
	ranking, _ := strconv.Atoi(gf.rankingEntry.Text)

	// Collect checked account IDs
	var accountIDs []string
	for i, check := range gf.memberChecks {
		if check.Checked && i < len(gf.allAccounts) {
			accountIDs = append(accountIDs, gf.allAccounts[i].ID)
		}
	}

	grp := &group.Group{
		Name:        gf.nameEntry.Text,
		Description: gf.descriptionEntry.Text,
		Ranking:     ranking,
		AccountIDs:  accountIDs,
	}

	// Preserve ID if editing
	if gf.current != nil {
		grp.ID = gf.current.ID
	}

	if gf.config.OnSave != nil {
		gf.config.OnSave(grp)
	}
}

func (gf *GroupForm) onDelete() {
	if gf.current != nil && gf.config.OnDelete != nil {
		gf.config.OnDelete(gf.current)
	}
}

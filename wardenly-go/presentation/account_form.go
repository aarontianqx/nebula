package presentation

import (
	"strconv"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/layout"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"wardenly-go/domain/account"
)

// AccountFormConfig holds configuration for AccountForm.
type AccountFormConfig struct {
	OnSave   func(*account.Account)
	OnDelete func(*account.Account)
}

// AccountForm provides a form for editing account details.
type AccountForm struct {
	config    *AccountFormConfig
	container *fyne.Container

	// Form fields
	roleNameEntry *widget.Entry
	userNameEntry *widget.Entry
	passwordEntry *widget.Entry
	serverIDEntry *widget.Entry
	rankingEntry  *widget.Entry

	// Buttons
	saveBtn   *widget.Button
	deleteBtn *widget.Button

	// Current account being edited
	current *account.Account
}

// NewAccountForm creates a new account editing form.
func NewAccountForm(cfg *AccountFormConfig) *AccountForm {
	af := &AccountForm{config: cfg}
	af.build()
	return af
}

func (af *AccountForm) build() {
	af.roleNameEntry = widget.NewEntry()
	af.roleNameEntry.SetPlaceHolder("Character name in game")

	af.userNameEntry = widget.NewEntry()
	af.userNameEntry.SetPlaceHolder("Login username")

	af.passwordEntry = widget.NewPasswordEntry()
	af.passwordEntry.SetPlaceHolder("Login password")

	af.serverIDEntry = widget.NewEntry()
	af.serverIDEntry.SetPlaceHolder("e.g., 126")

	af.rankingEntry = widget.NewEntry()
	af.rankingEntry.SetPlaceHolder("Sort priority (lower = higher)")

	// Use widget.Form for proper label-input alignment
	form := widget.NewForm(
		widget.NewFormItem("Role Name", af.roleNameEntry),
		widget.NewFormItem("User Name", af.userNameEntry),
		widget.NewFormItem("Password", af.passwordEntry),
		widget.NewFormItem("Server ID", af.serverIDEntry),
		widget.NewFormItem("Ranking", af.rankingEntry),
	)

	// Buttons with icons - Delete on left, Save on right
	af.deleteBtn = widget.NewButtonWithIcon("Delete", theme.DeleteIcon(), af.onDelete)
	af.deleteBtn.Importance = widget.DangerImportance

	af.saveBtn = widget.NewButtonWithIcon("Save", theme.DocumentSaveIcon(), af.onSave)
	af.saveBtn.Importance = widget.HighImportance

	buttonBar := container.NewHBox(
		af.deleteBtn,
		layout.NewSpacer(),
		af.saveBtn,
	)

	af.container = container.NewPadded(container.NewVBox(
		form,
		widget.NewSeparator(),
		buttonBar,
	))
}

// Container returns the form container.
func (af *AccountForm) Container() fyne.CanvasObject {
	return af.container
}

// SetAccount populates the form with account data.
// Pass nil to clear the form for creating a new account.
func (af *AccountForm) SetAccount(acc *account.Account) {
	af.current = acc

	if acc == nil {
		af.roleNameEntry.SetText("")
		af.userNameEntry.SetText("")
		af.passwordEntry.SetText("")
		af.serverIDEntry.SetText("")
		af.rankingEntry.SetText("0")
		af.deleteBtn.Disable()
	} else {
		af.roleNameEntry.SetText(acc.RoleName)
		af.userNameEntry.SetText(acc.UserName)
		af.passwordEntry.SetText(acc.Password)
		af.serverIDEntry.SetText(strconv.Itoa(acc.ServerID))
		af.rankingEntry.SetText(strconv.Itoa(acc.Ranking))
		af.deleteBtn.Enable()
	}
}

// Clear resets the form to empty state.
func (af *AccountForm) Clear() {
	af.SetAccount(nil)
}

func (af *AccountForm) onSave() {
	serverID, _ := strconv.Atoi(af.serverIDEntry.Text)
	ranking, _ := strconv.Atoi(af.rankingEntry.Text)

	acc := &account.Account{
		RoleName: af.roleNameEntry.Text,
		UserName: af.userNameEntry.Text,
		Password: af.passwordEntry.Text,
		ServerID: serverID,
		Ranking:  ranking,
	}

	// Preserve existing data if editing
	if af.current != nil {
		acc.ID = af.current.ID
		acc.Cookies = af.current.Cookies
	}

	if af.config.OnSave != nil {
		af.config.OnSave(acc)
	}
}

func (af *AccountForm) onDelete() {
	if af.current != nil && af.config.OnDelete != nil {
		af.config.OnDelete(af.current)
	}
}

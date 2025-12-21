package presentation

import (
	"context"
	"fmt"
	"log/slog"
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"

	"wardenly-go/domain/account"
	"wardenly-go/domain/group"
)

// ManagementDialogConfig holds configuration for the management dialog.
type ManagementDialogConfig struct {
	Parent         fyne.Window
	AccountService *account.Service
	GroupService   *group.Service
	Logger         *slog.Logger
	OnDataChanged  func() // Callback when data is modified
}

// ManagementDialog provides CRUD operations for accounts and groups.
type ManagementDialog struct {
	config *ManagementDialogConfig
	window fyne.Window

	// Tabs
	tabs *container.AppTabs

	// Accounts tab
	accountList     *widget.List
	accounts        []*account.Account
	selectedAccount *account.Account
	accountForm     *AccountForm

	// Groups tab
	groupList     *widget.List
	groups        []*group.Group
	selectedGroup *group.Group
	groupForm     *GroupForm
}

// ShowManagementDialog displays the account and group management dialog.
func ShowManagementDialog(cfg *ManagementDialogConfig) {
	if cfg.Logger == nil {
		cfg.Logger = slog.Default()
	}

	md := &ManagementDialog{
		config: cfg,
	}

	// Create a new window for the dialog
	app := cfg.Parent.Canvas().Content()
	_ = app // We need the parent's app

	md.window = fyne.CurrentApp().NewWindow("Account & Group Management")
	md.window.SetOnClosed(func() {
		// Nothing special to do on close
	})

	md.buildUI()
	md.loadData()

	md.window.Resize(fyne.NewSize(800, 600))
	md.window.CenterOnScreen()
	md.window.Show()
}

func (md *ManagementDialog) buildUI() {
	// Use native AppTabs for cleaner look
	accountsTab := container.NewTabItemWithIcon("Accounts", theme.AccountIcon(), md.buildAccountsTab())
	groupsTab := container.NewTabItemWithIcon("Groups", theme.FolderIcon(), md.buildGroupsTab())

	md.tabs = container.NewAppTabs(accountsTab, groupsTab)
	md.tabs.SetTabLocation(container.TabLocationTop)

	// Tabs fill the entire window - no bottom bar needed (window X button suffices)
	md.window.SetContent(md.tabs)
}

func (md *ManagementDialog) buildAccountsTab() fyne.CanvasObject {
	// New account button
	newBtn := widget.NewButtonWithIcon("New Account", theme.ContentAddIcon(), md.onNewAccount)
	newBtn.Importance = widget.HighImportance

	// Account list
	md.accountList = widget.NewList(
		func() int { return len(md.accounts) },
		func() fyne.CanvasObject {
			return widget.NewLabel("Template Account Name")
		},
		func(id widget.ListItemID, obj fyne.CanvasObject) {
			if id < len(md.accounts) {
				obj.(*widget.Label).SetText(md.accounts[id].Identity())
			}
		},
	)
	md.accountList.OnSelected = func(id widget.ListItemID) {
		if id < len(md.accounts) {
			md.selectedAccount = md.accounts[id]
			md.accountForm.SetAccount(md.selectedAccount)
		}
	}

	listPanel := container.NewBorder(
		container.NewVBox(newBtn, widget.NewSeparator()),
		nil, nil, nil,
		md.accountList,
	)

	// Account form
	md.accountForm = NewAccountForm(&AccountFormConfig{
		OnSave:   md.onSaveAccount,
		OnDelete: md.onDeleteAccount,
	})

	// Split layout
	split := container.NewHSplit(listPanel, md.accountForm.Container())
	split.SetOffset(0.35)

	return split
}

func (md *ManagementDialog) buildGroupsTab() fyne.CanvasObject {
	// New group button
	newBtn := widget.NewButtonWithIcon("New Group", theme.ContentAddIcon(), md.onNewGroup)
	newBtn.Importance = widget.HighImportance

	// Group list
	md.groupList = widget.NewList(
		func() int { return len(md.groups) },
		func() fyne.CanvasObject {
			return widget.NewLabel("Template Group Name (00)")
		},
		func(id widget.ListItemID, obj fyne.CanvasObject) {
			if id < len(md.groups) {
				grp := md.groups[id]
				obj.(*widget.Label).SetText(fmt.Sprintf("%s (%d)", grp.Name, grp.AccountCount()))
			}
		},
	)
	md.groupList.OnSelected = func(id widget.ListItemID) {
		if id < len(md.groups) {
			md.selectedGroup = md.groups[id]
			md.groupForm.SetGroup(md.selectedGroup, md.accounts)
		}
	}

	listPanel := container.NewBorder(
		container.NewVBox(newBtn, widget.NewSeparator()),
		nil, nil, nil,
		md.groupList,
	)

	// Group form
	md.groupForm = NewGroupForm(&GroupFormConfig{
		OnSave:   md.onSaveGroup,
		OnDelete: md.onDeleteGroup,
	})
	// Initialize with empty state
	md.groupForm.SetGroup(nil, md.accounts)

	// Split layout
	split := container.NewHSplit(listPanel, md.groupForm.Container())
	split.SetOffset(0.35)

	return split
}

func (md *ManagementDialog) loadData() {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	// Load accounts
	accounts, err := md.config.AccountService.ListAccounts(ctx)
	if err != nil {
		md.config.Logger.Error("Failed to load accounts", "error", err)
	} else {
		md.accounts = accounts
	}

	// Load groups
	groups, err := md.config.GroupService.ListAllGroups(ctx)
	if err != nil {
		md.config.Logger.Error("Failed to load groups", "error", err)
	} else {
		md.groups = groups
	}

	// Refresh lists
	if md.accountList != nil {
		md.accountList.Refresh()
	}
	if md.groupList != nil {
		md.groupList.Refresh()
	}
}

// Account handlers

func (md *ManagementDialog) onNewAccount() {
	md.selectedAccount = nil
	md.accountForm.SetAccount(nil)
	md.accountList.UnselectAll()
}

func (md *ManagementDialog) onSaveAccount(acc *account.Account) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	var err error
	if acc.ID == "" {
		err = md.config.AccountService.CreateAccount(ctx, acc)
	} else {
		err = md.config.AccountService.UpdateAccount(ctx, acc)
	}

	if err != nil {
		dialog.ShowError(err, md.window)
		return
	}

	md.loadData()
	md.notifyDataChanged()

	// Re-select the saved account if it was new
	if acc.ID != "" {
		for i, a := range md.accounts {
			if a.ID == acc.ID {
				md.accountList.Select(i)
				break
			}
		}
	}
}

func (md *ManagementDialog) onDeleteAccount(acc *account.Account) {
	if acc == nil || acc.ID == "" {
		return
	}

	dialog.ShowConfirm("Delete Account",
		fmt.Sprintf("Are you sure you want to delete account '%s'?\nThis will also remove it from all groups.", acc.Identity()),
		func(confirmed bool) {
			if !confirmed {
				return
			}

			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			defer cancel()

			// Delete account
			if err := md.config.AccountService.DeleteAccount(ctx, acc.ID); err != nil {
				dialog.ShowError(err, md.window)
				return
			}

			// Cleanup from groups
			if err := md.config.GroupService.CleanupAccountFromGroups(ctx, acc.ID); err != nil {
				md.config.Logger.Warn("Failed to cleanup account from groups", "error", err)
			}

			md.selectedAccount = nil
			md.accountForm.SetAccount(nil)
			md.loadData()
			md.notifyDataChanged()
		},
		md.window,
	)
}

// Group handlers

func (md *ManagementDialog) onNewGroup() {
	md.selectedGroup = nil
	md.groupForm.SetGroup(nil, md.accounts)
	md.groupList.UnselectAll()
}

func (md *ManagementDialog) onSaveGroup(grp *group.Group) {
	ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer cancel()

	var err error
	if grp.ID == "" {
		err = md.config.GroupService.CreateGroup(ctx, grp)
	} else {
		err = md.config.GroupService.UpdateGroup(ctx, grp)
	}

	if err != nil {
		dialog.ShowError(err, md.window)
		return
	}

	md.loadData()
	md.notifyDataChanged()

	// Re-select the saved group if it was new
	if grp.ID != "" {
		for i, g := range md.groups {
			if g.ID == grp.ID {
				md.groupList.Select(i)
				break
			}
		}
	}
}

func (md *ManagementDialog) onDeleteGroup(grp *group.Group) {
	if grp == nil || grp.ID == "" {
		return
	}

	dialog.ShowConfirm("Delete Group",
		fmt.Sprintf("Are you sure you want to delete group '%s'?", grp.Name),
		func(confirmed bool) {
			if !confirmed {
				return
			}

			ctx, cancel := context.WithTimeout(context.Background(), 10*time.Second)
			defer cancel()

			if err := md.config.GroupService.DeleteGroup(ctx, grp.ID); err != nil {
				dialog.ShowError(err, md.window)
				return
			}

			md.selectedGroup = nil
			md.groupForm.SetGroup(nil, md.accounts)
			md.loadData()
			md.notifyDataChanged()
		},
		md.window,
	)
}

func (md *ManagementDialog) notifyDataChanged() {
	if md.config.OnDataChanged != nil {
		md.config.OnDataChanged()
	}
}

# DNA VbCalc Application Ideas Preparation — 2026-03-09

Status: preparatory concept notes for a future separate `DnaVbCalc` repository  
Scope: preserve richer DNA VbCalc application ideas without letting them dominate OxVba workset planning.

## 1. Purpose

These are application-level ideas for the future DNA VbCalc project.

They are intentionally separate from:
1. OxVba workset execution,
2. the baseline host-shell definition,
3. the normative host/tooling contract.

## 2. Work panel concept

A richer future DNA VbCalc application could expose a work panel as an interactive surface with:
1. input controls:
   - text boxes,
   - spin buttons,
   - combo boxes,
   - check boxes
2. output controls:
   - labels,
   - list views,
   - simple grid/table views
3. action controls:
   - buttons,
   - toggle buttons
4. layout control:
   - add/remove/reposition controls,
   - host-driven and VBA-driven surface updates

This is explicitly beyond the first debug-shell baseline.

## 3. Rich host object model idea

A future richer object model could look like:

```text
Application
  ├── .Name
  ├── .Version
  ├── .ActiveWorkspace
  ├── .Quit()
  └── .WorkPanel
        ├── .Controls
        ├── .Refresh()
        └── .Clear()

Controls
  ├── .Count
  ├── .Item(index)
  ├── .Item(name)
  ├── .Add(...)
  └── .Remove(name)

Control
  ├── .Name
  ├── .Value
  ├── .Caption
  ├── .Visible
  ├── .Enabled
  └── events: Click, Change, DblClick, ...
```

This remains useful as a future target because it exercises:
1. root object injection,
2. child object navigation,
3. collection/default-member access,
4. property get/set,
5. method invocation,
6. event subscription and dispatch.

## 4. Event-driven UI example

Illustrative future scenario:

```vba
Private WithEvents btnCalc As Button
Private WithEvents txtInput As TextBox

Private Sub Workspace_Open()
    Set btnCalc = Application.WorkPanel.Controls("btnCalculate")
    Set txtInput = Application.WorkPanel.Controls("txtInput")
End Sub

Private Sub btnCalc_Click()
    Dim val As Double
    val = CDbl(txtInput.Text)
    Application.WorkPanel.Controls("lblResult").Caption = Format(val * 2, "#,##0.00")
End Sub
```

This is still a good stress case for the future host bridge, but it is not required for baseline v1.

## 5. Persistence-format idea

A future DNA VbCalc repository could use a host-owned container such as:
1. XML-in-ZIP `.vbcalc`,
2. or another host-controlled package format if implementation reality suggests something simpler.

The value of a host-owned container is:
1. embedded project storage,
2. workspace/layout persistence,
3. host-managed load/reload flows,
4. a concrete test of non-filesystem project sourcing.

## 6. Rich-host coverage checklist

The richer application ideas are still useful as a future checklist:
1. project load from host store
2. root object injection
3. object model navigation
4. event subscription lifecycle
5. event dispatch
6. error routing
7. multi-project support
8. export inventory
9. lifecycle management
10. policy/capability gating
11. add-in scope conversion
12. project reload

## 7. Illustrative future app sketch

For a richer future shell, an app loop could eventually look like:
1. create engine
2. construct host object model
3. register root objects
4. load host-managed project
5. compile and execute
6. run explicit event pump / dispatch loop

This is still a useful reference model, but it belongs with the future `DnaVbCalc` repo planning rather than OxVba worksets.

## 8. Relationship to the baseline

Baseline first:
1. Tauri shell
2. open `oxvba.toml`
3. debug/immediate-style surface
4. run/reset/eval/output
5. explicit host-event ingress

Potential later expansion:
1. work panel
2. richer controls
3. code-behind workspace model
4. embedded container format
5. broader interactive host object hierarchy

Sub Main()
    Dim app
    Dim oldVisible
    Dim oldDisplayAlerts

    app = CreateObject("Excel.Application")
    oldVisible = app.Visible
    oldDisplayAlerts = app.DisplayAlerts
    app.Visible = False
    app.DisplayAlerts = False
    app.DisplayAlerts = oldDisplayAlerts
    app.Visible = oldVisible
End Sub


Sub Main()
    Dim app
    Dim oldVisible

    app = CreateObject("Excel.Application")
    oldVisible = app.Visible
    app.Quit
End Sub

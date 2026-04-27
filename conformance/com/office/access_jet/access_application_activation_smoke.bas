Sub Main()
    Dim app
    Dim oldVisible

    app = CreateObject("Access.Application")
    oldVisible = DispatchInvoke(app, "Visible")
    DispatchInvoke(app, "Visible", False)
    DispatchInvoke(app, "Visible", oldVisible)
    DispatchInvoke(app, "Quit")
End Sub


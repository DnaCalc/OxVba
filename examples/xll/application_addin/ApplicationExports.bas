Public Function ExcelVersion() As String
    Dim app As Object
    Set app = Application.Value
    ExcelVersion = DispatchInvoke(app, "Version")
End Function

Public Function ExcelHwnd() As Double
    Dim app As Object
    Set app = Application.Value
    ExcelHwnd = DispatchInvoke(app, "Hwnd")
End Function

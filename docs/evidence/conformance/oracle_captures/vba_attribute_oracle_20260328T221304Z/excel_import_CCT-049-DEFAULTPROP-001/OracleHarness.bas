Attribute VB_Name = "OracleHarness"
Public Function RunProbe()
    On Error GoTo handler
    Dim widget As New Widget
    Dim valueOut
    valueOut = widget
    RunProbe = CStr(valueOut)
    Exit Function
handler:
    RunProbe = "ERR|" & CStr(Err.Number) & "|" & Err.Description
End Function

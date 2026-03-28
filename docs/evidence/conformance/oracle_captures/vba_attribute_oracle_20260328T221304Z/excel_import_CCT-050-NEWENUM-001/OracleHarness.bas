Attribute VB_Name = "OracleHarness"
Public Function RunProbe()
    On Error GoTo handler
    Dim widget As New Widget
    Dim item
    Dim acc
    For Each item In widget
        acc = acc & CStr(item) & ","
    Next item
    RunProbe = acc
    Exit Function
handler:
    RunProbe = "ERR|" & CStr(Err.Number) & "|" & Err.Description
End Function

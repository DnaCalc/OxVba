Public r As Variant

Sub Main()
    On Error Resume Next
    Dim x As Long
    x = CLng(2147483647.5)
    r = CStr(Err.Number) & ":" & CStr(x)
End Sub

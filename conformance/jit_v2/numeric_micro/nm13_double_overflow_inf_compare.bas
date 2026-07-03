Public r As Variant

Sub Main()
    On Error Resume Next
    Dim x As Double
    x = 1E+308 * 1E+308
    r = CStr(Err.Number) & ":" & CStr(x = x) & ":" & CStr(x > 0)
End Sub

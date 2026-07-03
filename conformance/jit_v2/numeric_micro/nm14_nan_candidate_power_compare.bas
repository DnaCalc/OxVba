Public r As Variant

Sub Main()
    On Error Resume Next
    Dim inf As Double
    Dim x As Double
    inf = 1E+308 * 1E+308
    x = inf / inf
    r = CStr(Err.Number) & ":" & CStr(x = x) & ":" & CStr(x <> x)
End Sub

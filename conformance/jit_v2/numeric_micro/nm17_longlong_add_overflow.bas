Public r As Variant

Sub Main()
    On Error Resume Next
    Dim x As LongLong
    x = &H7FFFFFFFFFFFFFFF^
    x = x + 1
    r = CStr(Err.Number) & ":" & CStr(x)
End Sub

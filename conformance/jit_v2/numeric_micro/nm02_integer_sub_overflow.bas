Public r As Variant

Sub Main()
    On Error Resume Next
    Dim x As Integer
    x = CInt(-32768)
    x = x - 1
    r = CStr(Err.Number) & ":" & CStr(x)
End Sub

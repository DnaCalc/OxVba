Public r As Variant

Sub Main()
    On Error Resume Next
    Dim a As Integer
    Dim b As Integer
    a = CInt(32766.5)
    b = CInt(32767.5)
    r = CStr(Err.Number) & ":" & CStr(a) & ":" & CStr(b)
End Sub

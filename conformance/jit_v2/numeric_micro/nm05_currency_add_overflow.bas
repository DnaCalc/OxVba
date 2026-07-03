Public r As Variant

Sub Main()
    On Error Resume Next
    Dim c As Currency
    c = CCur("922337203685477")
    c = c + CCur("1")
    r = CStr(Err.Number) & ":" & CStr(c)
End Sub

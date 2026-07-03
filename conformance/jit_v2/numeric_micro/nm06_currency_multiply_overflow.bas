Public r As Variant

Sub Main()
    On Error Resume Next
    Dim c As Currency
    c = CCur("30000000.0001") * CCur("30000000.0001")
    c = c * CCur("2")
    r = CStr(Err.Number) & ":" & CStr(c)
End Sub

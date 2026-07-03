Public r As Variant

Sub Main()
    Dim x As Variant
    x = CInt(32767)
    x = x + 1
    r = CStr(VarType(x)) & ":" & CStr(x)
End Sub

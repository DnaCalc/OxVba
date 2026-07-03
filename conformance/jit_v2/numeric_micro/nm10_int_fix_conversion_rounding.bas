Public r As Variant

Sub Main()
    r = CStr(Int(CDbl(-5.7))) & ":" & CStr(Fix(CDbl(-5.7))) & ":" & CStr(CLng(CDbl(-5.7)))
End Sub

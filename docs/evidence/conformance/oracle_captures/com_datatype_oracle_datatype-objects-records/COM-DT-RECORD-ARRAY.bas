Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
On Error Resume Next
v = DispatchInvoke(obj, "ReturnRecordArray")
If Err.Number <> 0 Then
    observed = "ERR:" & CStr(Err.Number)
Else
    observed = CStr(VarType(v)) & ":" & CStr(v)
End If
End Sub

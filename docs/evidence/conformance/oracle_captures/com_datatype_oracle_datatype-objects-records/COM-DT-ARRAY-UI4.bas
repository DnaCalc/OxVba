Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
On Error Resume Next
v = DispatchInvoke(obj, "ReturnUnsignedLongArray")
If Err.Number <> 0 Then
    observed = "ERR:" & CStr(Err.Number)
Else
    observed = CStr(VarType(v)) & ":" & CStr(LBound(v)) & ":" & CStr(UBound(v))
End If
End Sub

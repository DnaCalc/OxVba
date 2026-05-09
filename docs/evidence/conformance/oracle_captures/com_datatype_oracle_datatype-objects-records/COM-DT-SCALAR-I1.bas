Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
v = DispatchInvoke(obj, "ReturnSignedByteObject")
observed = CStr(VarType(v)) & ":" & CStr(v)
End Sub

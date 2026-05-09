Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
v = DispatchInvoke(obj, "ReturnDecimalObject")
observed = CStr(VarType(v)) & ":" & CStr(v)
End Sub

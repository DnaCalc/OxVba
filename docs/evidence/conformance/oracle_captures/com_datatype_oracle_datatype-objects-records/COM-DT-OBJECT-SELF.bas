Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
Set v = DispatchInvoke(obj, "ReturnSelfObject")
observed = CStr(VarType(v)) & ":" & CStr(DispatchInvoke(v, "Ping"))
End Sub

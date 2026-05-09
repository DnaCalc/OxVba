Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
v = DispatchInvoke(obj, "ReturnUnsignedLongObject")
observed = CStr(VarType(v)) & ":" & CStr(v)
End Sub

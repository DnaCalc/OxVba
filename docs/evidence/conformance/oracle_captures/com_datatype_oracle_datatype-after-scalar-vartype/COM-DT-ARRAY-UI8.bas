Sub Main()
Dim obj
Dim v
Dim observed
obj = CreateObject("OxVba.TestEventServer")
v = DispatchInvoke(obj, "ReturnUnsignedHyperArray")
observed = CStr(VarType(v)) & ":" & CStr(LBound(v)) & ":" & CStr(UBound(v))
End Sub

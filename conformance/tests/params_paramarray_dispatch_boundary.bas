Sub Main()
Dim x
Call InvokePack(x, 5, 7)
End Sub

Sub InvokePack(ByRef target, ParamArray items() As Variant)
target = DispatchInvoke(CreateObject(2), 4, items)
End Sub

Sub Main()
Dim x
Dim e
On Error Resume Next
x = DispatchInvoke(CreateObject("OxVba.TestDispatch"), "Count", 0)
e = Err.Number
End Sub

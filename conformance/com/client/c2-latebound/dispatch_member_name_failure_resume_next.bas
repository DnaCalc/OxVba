Sub Main()
Dim x
Dim e
On Error Resume Next
x = DispatchInvoke(CreateObject("Scripting.Dictionary"), "Count", 0)
e = Err.Number
End Sub

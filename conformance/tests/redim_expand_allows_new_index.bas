Sub Main()
Dim x
x = ProbeRedimExpand()
End Sub

Private Function ProbeRedimExpand()
Dim a()
ReDim a(3)
a(3) = 5
ProbeRedimExpand = a(3)
End Function

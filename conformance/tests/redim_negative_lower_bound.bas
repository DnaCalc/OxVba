Sub Main()
Dim x
x = ProbeRedimNegativeLowerBound()
End Sub

Private Function ProbeRedimNegativeLowerBound()
Dim a()
ReDim a(-2 To -1)
a(-2) = 5
a(-1) = 12
ProbeRedimNegativeLowerBound = a(-2) + a(-1)
End Function

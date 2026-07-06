Sub Main()
Dim x
x = ProbeRedimDynamicBoundExpression()
End Sub

Private Function ProbeRedimDynamicBoundExpression()
Dim a()
Dim n As Long
n = -2
ReDim a(n To n + 1)
a(-2) = 6
a(-1) = 13
ProbeRedimDynamicBoundExpression = a(-2) + a(-1)
End Function

Sub Main()
Dim x
x = ProbeRedimPreserveExplicitLowerKeepsValue()
End Sub

Private Function ProbeRedimPreserveExplicitLowerKeepsValue()
Dim a()
ReDim a(5 To 6)
a(6) = 8
ReDim Preserve a(5 To 8)
ProbeRedimPreserveExplicitLowerKeepsValue = a(6)
End Function

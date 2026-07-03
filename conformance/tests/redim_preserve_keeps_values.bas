Sub Main()
Dim x
x = ProbeRedimPreserveKeepsValues()
End Sub

Private Function ProbeRedimPreserveKeepsValues()
Dim a()
ReDim a(1)
a(0) = 7
ReDim Preserve a(3)
ProbeRedimPreserveKeepsValues = a(0)
End Function

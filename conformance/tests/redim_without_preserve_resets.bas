Sub Main()
Dim x
x = ProbeRedimWithoutPreserve()
End Sub

Private Function ProbeRedimWithoutPreserve()
Dim a()
ReDim a(1)
a(0) = 7
ReDim a(3)
ProbeRedimWithoutPreserve = a(0)
End Function

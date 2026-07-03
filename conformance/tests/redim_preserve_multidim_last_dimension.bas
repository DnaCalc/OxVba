Sub Main()
Dim x
x = ProbeRedimPreserveMultidim()
End Sub

Private Function ProbeRedimPreserveMultidim()
Dim m()
ReDim m(1 To 2, 1 To 2)
m(1, 1) = 7
ReDim Preserve m(1 To 2, 1 To 3)
ProbeRedimPreserveMultidim = m(1, 1)
End Function

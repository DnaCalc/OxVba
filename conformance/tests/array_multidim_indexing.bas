Sub Main()
Dim x
    x = ProbeMultidimIndexing()
End Sub

Private Function ProbeMultidimIndexing()
Dim m(1 To 2, 1 To 3)
m(2, 3) = 17
ProbeMultidimIndexing = m(2, 3)
End Function

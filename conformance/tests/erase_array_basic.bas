Sub Main()
    Dim x
    x = ProbeEraseArray()
End Sub

Private Function ProbeEraseArray()
    Dim a(2)
    a(0) = 7
    Erase a
    ProbeEraseArray = a(0)
End Function

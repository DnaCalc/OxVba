Sub Main()
    Dim x
    x = ProbeArrayStoreLoad()
End Sub

Private Function ProbeArrayStoreLoad()
    Dim a(2)
    a(0) = 5
    a(1) = 7
    ProbeArrayStoreLoad = a(1)
End Function

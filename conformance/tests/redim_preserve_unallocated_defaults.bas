Sub Main()
Dim x
x = ProbeRedimPreserveUnallocatedDefaults()
End Sub

Private Function ProbeRedimPreserveUnallocatedDefaults()
Dim a()
ReDim Preserve a(1 To 2)
ProbeRedimPreserveUnallocatedDefaults = a(2)
End Function

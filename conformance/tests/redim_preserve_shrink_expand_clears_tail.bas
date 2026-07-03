Sub Main()
Dim x
x = ProbeRedimShrinkExpandTail()
End Sub

Private Function ProbeRedimShrinkExpandTail()
Dim a()
ReDim a(0 To 3)
a(3) = 9
ReDim Preserve a(0 To 1)
ReDim Preserve a(0 To 3)
ProbeRedimShrinkExpandTail = a(3)
End Function

Type Point
    X As Integer
    Y As Integer
End Type

Sub Main()
    Dim x
    x = ProbePointFieldAccess()
End Sub

Private Function ProbePointFieldAccess()
    Dim p As Point
    p.X = 7
    p.Y = p.X
    ProbePointFieldAccess = p.Y
End Function

Type Point
    X As Integer
    Y As Integer
End Type

Sub Main()
    Dim ax
    Dim ay
    Dim bx
    Dim by
    Dim x
    ProbePointWholeCopy ax, ay, bx, by, x
End Sub

Private Sub ProbePointWholeCopy(ByRef ax, ByRef ay, ByRef bx, ByRef by, ByRef x)
    Dim a As Point
    Dim b As Point
    a.X = 7
    a.Y = 9
    b = a
    ax = a.X
    ay = a.Y
    bx = b.X
    by = b.Y
    x = b.Y
End Sub

Type Point
    X As Long
    Y As Long
End Type

Sub Main()
    Dim a As Point
    Dim b As Point
    Dim total As Long
    Dim shifted As Long

    a.X = 7
    a.Y = 9
    b = a
    b.X = b.X + 3
    total = b.X + b.Y
    shifted = a.X + b.X
End Sub

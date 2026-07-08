Private Type Point
    X As Long
    Y As Long
    Label As String * 4
End Type

Public result As Long

Sub Main()
    Dim p As Point
    Dim q As Point
    Dim i As Long

    p.Label = "abcd"
    For i = 1 To 2000
        p.X = p.X + 1
        p.Y = p.Y + p.X
        q = p
        result = result + q.X + Len(q.Label)
    Next i
End Sub

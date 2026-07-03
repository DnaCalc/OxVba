Public result As Long

Sub Main()
    Dim i As Long
    Dim c As New Collection
    Dim v As Variant

    For i = 1 To 7000
        c.Add i
    Next i

    For i = 1 To 7000
        v = c.Item(1)
        result = result + v
        c.Remove 1
    Next i
End Sub

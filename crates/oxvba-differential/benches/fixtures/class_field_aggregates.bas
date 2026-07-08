Public result As Long

Sub Main()
    Dim box As Box

    Set box = New Box
    box.Fill 900
    result = box.Sum()
End Sub

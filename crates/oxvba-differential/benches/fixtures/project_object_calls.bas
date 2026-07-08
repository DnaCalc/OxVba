Public result As Long

Sub Main()
    Dim c As Counter
    Dim i As Long

    Set c = New Counter
    For i = 1 To 3500
        c.Add i Mod 9
        result = result + c.Value
    Next i
End Sub

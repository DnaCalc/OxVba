Public result As Long

Sub Main()
    Dim c As Object
    Dim i As Long

    Set c = New Counter
    For i = 1 To 2500
        CallByName c, "Add", VbMethod, i Mod 11
        result = result + CallByName(c, "Value", VbGet)
    Next i
End Sub

Public result As Long

Sub Main()
    Dim i As Long
    Dim s As String
    Dim builder As String

    For i = 1 To 1000
        s = s & "x"
    Next i

    builder = Space(1000)
    For i = 1 To 1000
        Mid(builder, i, 1) = "y"
    Next i

    result = Len(s) + Len(builder)
End Sub

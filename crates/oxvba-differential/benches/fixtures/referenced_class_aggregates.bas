Public result As Long

Sub Main()
    Dim box As Lib.Box

    Set box = New Lib.Box
    box.Fill 700
    result = box.Sum()
End Sub

Public result As Long

Sub Main()
    Dim item As Variant
    Dim v As Variant

    v = Array(1, 2, 3, 4, 5, 6, 7, 8)
    For Each item In v
        result = result + item
    Next item
End Sub

Sub Main()
    Dim score As Long
    Dim expected As Long
    Dim item
    Dim a(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Long

    a(1, 1, 1, 1) = 1
    a(1, 1, 1, 2) = 2
    a(1, 1, 2, 1) = 3
    a(1, 1, 2, 2) = 4
    a(1, 2, 1, 1) = 5
    a(1, 2, 1, 2) = 6
    a(1, 2, 2, 1) = 7
    a(1, 2, 2, 2) = 8
    a(2, 1, 1, 1) = 9
    a(2, 1, 1, 2) = 10
    a(2, 1, 2, 1) = 11
    a(2, 1, 2, 2) = 12
    a(2, 2, 1, 1) = 13
    a(2, 2, 1, 2) = 14
    a(2, 2, 2, 1) = 15
    a(2, 2, 2, 2) = 16

    For Each item In a
        expected = expected + 1
        If item = expected Then
            score = score + 1
        Else
            score = score - 100
        End If
    Next

    If expected <> 16 Then score = score - 1000
End Sub

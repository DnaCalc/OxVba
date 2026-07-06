Sub Main()
    Dim observed As Double
    Dim item
    Dim a(1 To 2, 1 To 3) As Boolean
    a(1, 1) = False
    a(1, 2) = True
    a(1, 3) = True
    a(2, 1) = False
    a(2, 2) = True
    a(2, 3) = False
    For Each item In a
        If item Then
            observed = observed * 10# + 2#
        Else
            observed = observed * 10# + 1#
        End If
    Next
End Sub

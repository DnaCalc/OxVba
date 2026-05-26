Sub Main()
    Dim s As String
    Dim t As String
    Dim n As Long

    s = "alpha"
    If Len(s) = 5 Then
        t = s & "-beta"
    Else
        t = s & "-bad"
    End If
    n = Len(t)
End Sub

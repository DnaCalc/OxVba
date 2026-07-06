Sub Main()
    Dim item
    Dim source As String * 3
    source = "abc"
    For Each item In source
        source = "xyz"
    Next
End Sub

Sub Main()
    Dim item
    Dim source As Byte
    source = CByte(5)
    For Each item In source
        source = CByte(9)
    Next
End Sub

Public result As Long

Sub Main()
    Dim i As Long
    Dim n As Long

    For i = 1 To 25000
        n = AddByVal(n, i)
        BumpByRef n
    Next i

    result = n
End Sub

Private Function AddByVal(ByVal left As Long, ByVal right As Long) As Long
    AddByVal = left + (right Mod 7)
End Function

Private Sub BumpByRef(ByRef value As Long)
    value = value + 1
End Sub

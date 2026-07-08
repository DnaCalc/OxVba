Private Type Cell
    Values(0 To 1) As Long
End Type

Private cells() As Cell

Public Sub Fill(ByVal count As Long)
    Dim i As Long

    ReDim cells(0 To count - 1)
    For i = 0 To count - 1
        cells(i).Values(0) = i Mod 113
        cells(i).Values(1) = cells(i).Values(0) + 3
    Next i
End Sub

Public Function Sum() As Long
    Dim i As Long

    For i = LBound(cells) To UBound(cells)
        Sum = Sum + cells(i).Values(1)
    Next i
End Function

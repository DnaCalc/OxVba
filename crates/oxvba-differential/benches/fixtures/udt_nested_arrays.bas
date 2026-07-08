Private Type Cell
    Values(0 To 1) As Long
End Type

Private Type Row
    Cells(0 To 1) As Cell
End Type

Public result As Long

Sub Main()
    Dim rows() As Row
    Dim i As Long

    ReDim rows(0 To 799)
    For i = 0 To 799
        rows(i).Cells(0).Values(0) = i Mod 97
        rows(i).Cells(0).Values(1) = rows(i).Cells(0).Values(0) + 1
        rows(i).Cells(1).Values(0) = rows(i).Cells(0).Values(1) + 1
        rows(i).Cells(1).Values(1) = rows(i).Cells(1).Values(0) + 1
        result = result + rows(i).Cells(1).Values(1)
    Next i
End Sub

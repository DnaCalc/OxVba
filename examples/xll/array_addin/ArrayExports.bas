Public Function MakeVector() As Variant
    MakeVector = Array(10, 20, 30)
End Function

Public Function SumArray(ByVal values As Variant) As Long
    Dim item
    For Each item In values
        SumArray = SumArray + item
    Next
End Function

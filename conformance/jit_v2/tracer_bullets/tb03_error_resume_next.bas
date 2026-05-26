Sub Main()
    Dim beforeErr As Long
    Dim afterErr As Long
    Dim numerator As Long
    Dim denominator As Long
    Dim result As Double

    numerator = 10
    denominator = 0
    beforeErr = Err.Number
    On Error Resume Next
    result = numerator / denominator
    afterErr = Err.Number
End Sub

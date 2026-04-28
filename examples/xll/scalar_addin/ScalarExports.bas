Public Function AddDouble(ByVal x As Double, ByVal y As Double) As Double
    AddDouble = x + y
End Function

Public Function EchoText(ByVal s As String) As String
    EchoText = s
End Function

Public Function NotFlag(ByVal b As Boolean) As Boolean
    If b Then
        NotFlag = False
    Else
        NotFlag = True
    End If
End Function

Public Function IncLong(ByVal n As Long) As Long
    IncLong = n + 1
End Function

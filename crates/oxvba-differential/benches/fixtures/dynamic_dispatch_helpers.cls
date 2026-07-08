Private m As Long

Public Sub Add(ByVal value As Long)
    m = m + value
End Sub

Public Property Get Value() As Long
    Value = m
End Property

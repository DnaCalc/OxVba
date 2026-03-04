Option Explicit

Public Function Score() As Long
Dim items() As Long
ReDim items(0 To 2)
items(0) = 1
items(1) = 2
items(2) = 7
Score = items(2)
End Function

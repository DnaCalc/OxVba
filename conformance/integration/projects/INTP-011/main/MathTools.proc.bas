Option Explicit

Public Function SumUpTo(ByVal limit As Long) As Long
Dim i As Long
Dim acc As Long
acc = 6
For i = 1 To limit
acc = acc + 1
Next i
SumUpTo = acc
End Function

Option Explicit

Public Function SafeAdd(ByVal a As Long, ByVal b As Long) As Long
SafeAdd = a + b
End Function

Public Sub MayFail(ByVal flag As Long)
If flag = 1 Then
    Err.Raise 42
End If
End Sub

Sub Main()
Dim seed As Long
Dim aliasObserved As Long
Dim forcedByValObserved As Long
Dim expressionObserved As Long
Dim literalSurvived As Long
Dim functionSurvived As Long

seed = 10
Call Touch(seed)
aliasObserved = seed

seed = 10
Touch (seed)
forcedByValObserved = seed

seed = 10
Call Touch(seed + 5)
expressionObserved = seed

Call Touch(3)
literalSurvived = 1

Call Touch(MakeLong())
functionSurvived = 1
End Sub

Sub Touch(ByRef target As Long)
target = target + 1
End Sub

Function MakeLong() As Long
MakeLong = 21
End Function

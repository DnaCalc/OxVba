Public text As String

Sub Main()
text = "before"
Call Mutate((text))
End Sub

Sub Mutate(ParamArray items() As Variant)
items(0) = "global"
End Sub

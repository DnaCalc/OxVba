Attribute VB_Name = "EngineeringMath"

Public Function AddLongs(ByVal leftValue As Long, ByVal rightValue As Long) As Long
    AddLongs = leftValue + rightValue
End Function

Public Function Hypotenuse(ByVal width As Double, ByVal height As Double) As Double
    Hypotenuse = Sqr(width * width + height * height)
End Function

Public Function ScaleLoad(ByVal load As Double, ByVal factor As Double) As Double
    ScaleLoad = load * factor
End Function

Private Function InternalOffset(ByVal value As Long) As Long
    InternalOffset = value + 100
End Function

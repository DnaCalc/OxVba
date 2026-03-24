Enum Color
    Red = 1
    Green = 2
    Blue = 3
End Enum

Sub Main()
    Dim x
    Dim c
    c = Green
    Select Case c
        Case Red
            x = 10
        Case Green
            x = 20
        Case Blue
            x = 30
    End Select
End Sub

Sub Main()
    Dim a(2) As Long
    Dim arr
    Dim v As Long
    Dim first As Long
    Dim third As Long
    Dim lower As Long
    Dim upper As Long
    Dim total As Long

    a(0) = 2
    a(1) = 3
    a(2) = 5
    first = a(0)
    third = a(2)
    arr = Array(2, 3, 5)
    lower = LBound(arr)
    upper = UBound(arr)

    For Each v In a
        total = total + v
    Next
End Sub

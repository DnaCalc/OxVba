Public result As Long

Sub Main()
    Dim i As Long
    Dim late As Object
    Dim early As OxVba.TestDispatch
    Dim lateHits As Long
    Dim earlyHits As Long

    For i = 1 To 11500
        Set late = CreateObject("OxVba.TestDispatch")
        If Not late Is Nothing Then
            lateHits = lateHits + 1
        End If
    Next i

    For i = 1 To 11500
        Set early = New OxVba.TestDispatch
        If Not early Is Nothing Then
            earlyHits = earlyHits + 1
        End If
    Next i

    result = lateHits + earlyHits
End Sub

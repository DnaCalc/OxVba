Sub Main()
    Dim score As Long
    Dim expected As Long
    Dim i As Long
    Dim j As Long
    Dim k As Long
    Dim l As Long
    Dim n As Long
    Dim ok As Boolean
    Dim item
    Dim abool(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Boolean
    Dim abyte(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Byte
    Dim ai(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Integer
    Dim al(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Long
    Dim all(1 To 2, 1 To 2, 1 To 2, 1 To 2) As LongLong
    Dim asng(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Single
    Dim adbl(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Double
    Dim acur(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Currency
    Dim adate(1 To 2, 1 To 2, 1 To 2, 1 To 2) As Date
    Dim astr(1 To 2, 1 To 2, 1 To 2, 1 To 2) As String

    For i = 1 To 2
        For j = 1 To 2
            For k = 1 To 2
                For l = 1 To 2
                    n = n + 1
                    abool(i, j, k, l) = ((n Mod 2) = 0)
                    abyte(i, j, k, l) = n
                    ai(i, j, k, l) = n
                    al(i, j, k, l) = n
                    all(i, j, k, l) = n
                    asng(i, j, k, l) = n
                    adbl(i, j, k, l) = n
                    acur(i, j, k, l) = n
                    adate(i, j, k, l) = CDate(CDbl(n))
                    astr(i, j, k, l) = CStr(n)
                Next
            Next
        Next
    Next

    expected = 0
    ok = True
    For Each item In abool
        expected = expected + 1
        If CBool(item) <> ((expected Mod 2) = 0) Then ok = False
    Next
    If ok And expected = 16 Then score = score + 1

    expected = 0
    ok = True
    For Each item In abyte
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 2

    expected = 0
    ok = True
    For Each item In ai
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 4

    expected = 0
    ok = True
    For Each item In al
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 8

    expected = 0
    ok = True
    For Each item In all
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 16

    expected = 0
    ok = True
    For Each item In asng
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 32

    expected = 0
    ok = True
    For Each item In adbl
        expected = expected + 1
        If item <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 64

    expected = 0
    ok = True
    For Each item In acur
        expected = expected + 1
        If CDbl(item) <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 128

    expected = 0
    ok = True
    For Each item In adate
        expected = expected + 1
        If CDbl(item) <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 256

    expected = 0
    ok = True
    For Each item In astr
        expected = expected + 1
        If CDbl(item) <> expected Then ok = False
    Next
    If ok And expected = 16 Then score = score + 512
End Sub

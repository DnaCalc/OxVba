Sub Main()
    Dim score As Long
    Dim observed As Double
    Dim item
    Dim abool() As Boolean
    Dim abyte() As Byte
    Dim ai() As Integer
    Dim al() As Long
    Dim all() As LongLong
    Dim asng() As Single
    Dim adbl() As Double
    Dim acur() As Currency
    Dim adate() As Date
    Dim astr() As String

    ReDim abool(1 To 2, 1 To 2, 1 To 2)
    abool(1, 1, 1) = False
    abool(1, 1, 2) = True
    abool(1, 2, 1) = False
    abool(1, 2, 2) = True
    abool(2, 1, 1) = True
    abool(2, 1, 2) = False
    abool(2, 2, 1) = True
    abool(2, 2, 2) = False
    observed = 0#
    For Each item In abool
        If item Then
            observed = observed * 10# + 2#
        Else
            observed = observed * 10# + 1#
        End If
    Next
    If observed = 12122121# Then score = score + 1

    ReDim abyte(1 To 2, 1 To 2, 1 To 2)
    abyte(1, 1, 1) = 1
    abyte(1, 1, 2) = 2
    abyte(1, 2, 1) = 3
    abyte(1, 2, 2) = 4
    abyte(2, 1, 1) = 5
    abyte(2, 1, 2) = 6
    abyte(2, 2, 1) = 7
    abyte(2, 2, 2) = 8
    observed = 0#
    For Each item In abyte
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 2

    ReDim ai(1 To 2, 1 To 2, 1 To 2)
    ai(1, 1, 1) = 1
    ai(1, 1, 2) = 2
    ai(1, 2, 1) = 3
    ai(1, 2, 2) = 4
    ai(2, 1, 1) = 5
    ai(2, 1, 2) = 6
    ai(2, 2, 1) = 7
    ai(2, 2, 2) = 8
    observed = 0#
    For Each item In ai
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 4

    ReDim al(1 To 2, 1 To 2, 1 To 2)
    al(1, 1, 1) = 1
    al(1, 1, 2) = 2
    al(1, 2, 1) = 3
    al(1, 2, 2) = 4
    al(2, 1, 1) = 5
    al(2, 1, 2) = 6
    al(2, 2, 1) = 7
    al(2, 2, 2) = 8
    observed = 0#
    For Each item In al
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 8

    ReDim all(1 To 2, 1 To 2, 1 To 2)
    all(1, 1, 1) = 1^
    all(1, 1, 2) = 2^
    all(1, 2, 1) = 3^
    all(1, 2, 2) = 4^
    all(2, 1, 1) = 5^
    all(2, 1, 2) = 6^
    all(2, 2, 1) = 7^
    all(2, 2, 2) = 8^
    observed = 0#
    For Each item In all
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 16

    ReDim asng(1 To 2, 1 To 2, 1 To 2)
    asng(1, 1, 1) = 1!
    asng(1, 1, 2) = 2!
    asng(1, 2, 1) = 3!
    asng(1, 2, 2) = 4!
    asng(2, 1, 1) = 5!
    asng(2, 1, 2) = 6!
    asng(2, 2, 1) = 7!
    asng(2, 2, 2) = 8!
    observed = 0#
    For Each item In asng
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 32

    ReDim adbl(1 To 2, 1 To 2, 1 To 2)
    adbl(1, 1, 1) = 1#
    adbl(1, 1, 2) = 2#
    adbl(1, 2, 1) = 3#
    adbl(1, 2, 2) = 4#
    adbl(2, 1, 1) = 5#
    adbl(2, 1, 2) = 6#
    adbl(2, 2, 1) = 7#
    adbl(2, 2, 2) = 8#
    observed = 0#
    For Each item In adbl
        observed = observed * 10# + item
    Next
    If observed = 12345678# Then score = score + 64

    ReDim acur(1 To 2, 1 To 2, 1 To 2)
    acur(1, 1, 1) = 1@
    acur(1, 1, 2) = 2@
    acur(1, 2, 1) = 3@
    acur(1, 2, 2) = 4@
    acur(2, 1, 1) = 5@
    acur(2, 1, 2) = 6@
    acur(2, 2, 1) = 7@
    acur(2, 2, 2) = 8@
    observed = 0#
    For Each item In acur
        observed = observed * 10# + CDbl(item)
    Next
    If observed = 12345678# Then score = score + 128

    ReDim adate(1 To 2, 1 To 2, 1 To 2)
    adate(1, 1, 1) = CDate(1#)
    adate(1, 1, 2) = CDate(2#)
    adate(1, 2, 1) = CDate(3#)
    adate(1, 2, 2) = CDate(4#)
    adate(2, 1, 1) = CDate(5#)
    adate(2, 1, 2) = CDate(6#)
    adate(2, 2, 1) = CDate(7#)
    adate(2, 2, 2) = CDate(8#)
    observed = 0#
    For Each item In adate
        observed = observed * 10# + CDbl(item)
    Next
    If observed = 12345678# Then score = score + 256

    ReDim astr(1 To 2, 1 To 2, 1 To 2)
    astr(1, 1, 1) = "1"
    astr(1, 1, 2) = "2"
    astr(1, 2, 1) = "3"
    astr(1, 2, 2) = "4"
    astr(2, 1, 1) = "5"
    astr(2, 1, 2) = "6"
    astr(2, 2, 1) = "7"
    astr(2, 2, 2) = "8"
    observed = 0#
    For Each item In astr
        observed = observed * 10# + CDbl(item)
    Next
    If observed = 12345678# Then score = score + 512
End Sub

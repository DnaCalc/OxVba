Sub Main()
    Dim score As Long
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
    ReDim abyte(1 To 2, 1 To 2, 1 To 2)
    ReDim ai(1 To 2, 1 To 2, 1 To 2)
    ReDim al(1 To 2, 1 To 2, 1 To 2)
    ReDim all(1 To 2, 1 To 2, 1 To 2)
    ReDim asng(1 To 2, 1 To 2, 1 To 2)
    ReDim adbl(1 To 2, 1 To 2, 1 To 2)
    ReDim acur(1 To 2, 1 To 2, 1 To 2)
    ReDim adate(1 To 2, 1 To 2, 1 To 2)
    ReDim astr(1 To 2, 1 To 2, 1 To 2)

    For Each item In abool
    Next
    If IsEmpty(item) Then score = score + 1

    For Each item In abyte
    Next
    If IsEmpty(item) Then score = score + 2

    For Each item In ai
    Next
    If IsEmpty(item) Then score = score + 4

    For Each item In al
    Next
    If IsEmpty(item) Then score = score + 8

    For Each item In all
    Next
    If IsEmpty(item) Then score = score + 16

    For Each item In asng
    Next
    If IsEmpty(item) Then score = score + 32

    For Each item In adbl
    Next
    If IsEmpty(item) Then score = score + 64

    For Each item In acur
    Next
    If IsEmpty(item) Then score = score + 128

    For Each item In adate
    Next
    If IsEmpty(item) Then score = score + 256

    For Each item In astr
    Next
    If IsEmpty(item) Then score = score + 512
End Sub

Sub Main()
    Dim score As Long
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

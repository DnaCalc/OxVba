Sub Main()
Dim score As Long
Dim ab(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Boolean
Dim abyte(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Byte
Dim ai(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Integer
Dim al(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Long
Dim all(1 To 2, 1 To 3, 1 To 4, 1 To 5) As LongLong
Dim asg(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Single
Dim adbl(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Double
Dim acur(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Currency
Dim adate(1 To 2, 1 To 3, 1 To 4, 1 To 5) As Date
Dim astr(1 To 2, 1 To 3, 1 To 4, 1 To 5) As String
ab(2, 3, 4, 5) = True
abyte(2, 3, 4, 5) = CByte(7)
ai(2, 3, 4, 5) = 44%
al(2, 3, 4, 5) = 42&
all(2, 3, 4, 5) = 5000000012^
asg(2, 3, 4, 5) = 1.25!
adbl(2, 3, 4, 5) = 2.5#
acur(2, 3, 4, 5) = 12.3456@
adate(2, 3, 4, 5) = CDate(36527#)
astr(2, 3, 4, 5) = "alpha"
Erase ab
Erase abyte
Erase ai
Erase al
Erase all
Erase asg
Erase adbl
Erase acur
Erase adate
Erase astr
If ab(2, 3, 4, 5) = False Then score = score + 1
If abyte(2, 3, 4, 5) = CByte(0) Then score = score + 2
If ai(2, 3, 4, 5) = 0% Then score = score + 4
If al(2, 3, 4, 5) = 0& Then score = score + 8
If all(2, 3, 4, 5) = 0^ Then score = score + 16
If asg(2, 3, 4, 5) = 0! Then score = score + 32
If adbl(2, 3, 4, 5) = 0# Then score = score + 64
If acur(2, 3, 4, 5) = 0@ Then score = score + 128
If adate(2, 3, 4, 5) = CDate(0#) Then score = score + 256
If Len(astr(2, 3, 4, 5)) = 0 Then score = score + 512
End Sub

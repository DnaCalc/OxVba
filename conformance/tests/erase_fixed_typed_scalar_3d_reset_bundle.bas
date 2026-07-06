Sub Main()
Dim score As Long
Dim ab(1 To 2, 1 To 3, 1 To 4) As Boolean
Dim abyte(1 To 2, 1 To 3, 1 To 4) As Byte
Dim ai(1 To 2, 1 To 3, 1 To 4) As Integer
Dim al(1 To 2, 1 To 3, 1 To 4) As Long
Dim all(1 To 2, 1 To 3, 1 To 4) As LongLong
Dim asg(1 To 2, 1 To 3, 1 To 4) As Single
Dim adbl(1 To 2, 1 To 3, 1 To 4) As Double
Dim acur(1 To 2, 1 To 3, 1 To 4) As Currency
Dim adate(1 To 2, 1 To 3, 1 To 4) As Date
Dim astr(1 To 2, 1 To 3, 1 To 4) As String
ab(2, 3, 4) = True
abyte(2, 3, 4) = CByte(7)
ai(2, 3, 4) = 44%
al(2, 3, 4) = 42&
all(2, 3, 4) = 5000000012^
asg(2, 3, 4) = 1.25!
adbl(2, 3, 4) = 2.5#
acur(2, 3, 4) = 12.3456@
adate(2, 3, 4) = CDate(36527#)
astr(2, 3, 4) = "alpha"
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
If ab(2, 3, 4) = False Then score = score + 1
If abyte(2, 3, 4) = CByte(0) Then score = score + 2
If ai(2, 3, 4) = 0% Then score = score + 4
If al(2, 3, 4) = 0& Then score = score + 8
If all(2, 3, 4) = 0^ Then score = score + 16
If asg(2, 3, 4) = 0! Then score = score + 32
If adbl(2, 3, 4) = 0# Then score = score + 64
If acur(2, 3, 4) = 0@ Then score = score + 128
If adate(2, 3, 4) = CDate(0#) Then score = score + 256
If Len(astr(2, 3, 4)) = 0 Then score = score + 512
End Sub

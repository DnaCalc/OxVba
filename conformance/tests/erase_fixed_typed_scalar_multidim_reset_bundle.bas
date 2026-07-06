Sub Main()
Dim score As Long
Dim ab(1 To 2, 1 To 3) As Boolean
Dim abyte(1 To 2, 1 To 3) As Byte
Dim ai(1 To 2, 1 To 3) As Integer
Dim al(1 To 2, 1 To 3) As Long
Dim all(1 To 2, 1 To 3) As LongLong
Dim asg(1 To 2, 1 To 3) As Single
Dim adbl(1 To 2, 1 To 3) As Double
Dim acur(1 To 2, 1 To 3) As Currency
Dim adate(1 To 2, 1 To 3) As Date
Dim astr(1 To 2, 1 To 3) As String
ab(2, 3) = True
abyte(2, 3) = CByte(7)
ai(2, 3) = 44%
al(2, 3) = 42&
all(2, 3) = 5000000012^
asg(2, 3) = 1.25!
adbl(2, 3) = 2.5#
acur(2, 3) = 12.3456@
adate(2, 3) = CDate(36527#)
astr(2, 3) = "alpha"
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
If ab(2, 3) = False Then score = score + 1
If abyte(2, 3) = CByte(0) Then score = score + 2
If ai(2, 3) = 0% Then score = score + 4
If al(2, 3) = 0& Then score = score + 8
If all(2, 3) = 0^ Then score = score + 16
If asg(2, 3) = 0! Then score = score + 32
If adbl(2, 3) = 0# Then score = score + 64
If acur(2, 3) = 0@ Then score = score + 128
If adate(2, 3) = CDate(0#) Then score = score + 256
If Len(astr(2, 3)) = 0 Then score = score + 512
End Sub

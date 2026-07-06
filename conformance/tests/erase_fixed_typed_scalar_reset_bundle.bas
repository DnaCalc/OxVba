Sub Main()
Dim score As Long
Dim ab(0) As Boolean
Dim abyte(0) As Byte
Dim ai(0) As Integer
Dim al(0) As Long
Dim all(0) As LongLong
Dim asg(0) As Single
Dim adbl(0) As Double
Dim acur(0) As Currency
Dim adate(0) As Date
Dim astr(0) As String
ab(0) = True
abyte(0) = CByte(7)
ai(0) = 44%
al(0) = 42&
all(0) = 5000000012^
asg(0) = 1.25!
adbl(0) = 2.5#
acur(0) = 12.3456@
adate(0) = CDate(36527#)
astr(0) = "alpha"
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
If ab(0) = False Then score = score + 1
If abyte(0) = CByte(0) Then score = score + 2
If ai(0) = 0% Then score = score + 4
If al(0) = 0& Then score = score + 8
If all(0) = 0^ Then score = score + 16
If asg(0) = 0! Then score = score + 32
If adbl(0) = 0# Then score = score + 64
If acur(0) = 0@ Then score = score + 128
If adate(0) = CDate(0#) Then score = score + 256
If Len(astr(0)) = 0 Then score = score + 512
End Sub

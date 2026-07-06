Sub Main()
Dim gotBool As Boolean
Dim gotByte As Byte
Dim gotInteger As Integer
Dim gotLong As Long
Dim gotLongLong As LongLong
Dim gotSingle As Single
Dim gotDouble As Double
Dim gotCurrency As Currency
Dim gotDate As Date
Dim gotString As String
Dim ab() As Boolean
Dim abyte() As Byte
Dim ai() As Integer
Dim al() As Long
Dim all() As LongLong
Dim asg() As Single
Dim adbl() As Double
Dim acur() As Currency
Dim adate() As Date
Dim astr() As String
ReDim ab(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim abyte(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim ai(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim al(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim all(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim asg(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim adbl(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim acur(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim adate(1 To 2, 1 To 3, 1 To 4, 1 To 5)
ReDim astr(1 To 2, 1 To 3, 1 To 4, 1 To 5)
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
gotBool = ab(2, 3, 4, 5)
gotByte = abyte(2, 3, 4, 5)
gotInteger = ai(2, 3, 4, 5)
gotLong = al(2, 3, 4, 5)
gotLongLong = all(2, 3, 4, 5)
gotSingle = asg(2, 3, 4, 5)
gotDouble = adbl(2, 3, 4, 5)
gotCurrency = acur(2, 3, 4, 5)
gotDate = adate(2, 3, 4, 5)
gotString = astr(2, 3, 4, 5)
End Sub

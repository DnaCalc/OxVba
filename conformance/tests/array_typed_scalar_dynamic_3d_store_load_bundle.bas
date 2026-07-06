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
ReDim ab(1 To 2, 1 To 3, 1 To 4)
ReDim abyte(1 To 2, 1 To 3, 1 To 4)
ReDim ai(1 To 2, 1 To 3, 1 To 4)
ReDim al(1 To 2, 1 To 3, 1 To 4)
ReDim all(1 To 2, 1 To 3, 1 To 4)
ReDim asg(1 To 2, 1 To 3, 1 To 4)
ReDim adbl(1 To 2, 1 To 3, 1 To 4)
ReDim acur(1 To 2, 1 To 3, 1 To 4)
ReDim adate(1 To 2, 1 To 3, 1 To 4)
ReDim astr(1 To 2, 1 To 3, 1 To 4)
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
gotBool = ab(2, 3, 4)
gotByte = abyte(2, 3, 4)
gotInteger = ai(2, 3, 4)
gotLong = al(2, 3, 4)
gotLongLong = all(2, 3, 4)
gotSingle = asg(2, 3, 4)
gotDouble = adbl(2, 3, 4)
gotCurrency = acur(2, 3, 4)
gotDate = adate(2, 3, 4)
gotString = astr(2, 3, 4)
End Sub

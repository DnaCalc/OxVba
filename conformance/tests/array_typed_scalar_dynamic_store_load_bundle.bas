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
ReDim ab(0)
ReDim abyte(0)
ReDim ai(0)
ReDim al(0)
ReDim all(0)
ReDim asg(0)
ReDim adbl(0)
ReDim acur(0)
ReDim adate(0)
ReDim astr(0)
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
gotBool = ab(0)
gotByte = abyte(0)
gotInteger = ai(0)
gotLong = al(0)
gotLongLong = all(0)
gotSingle = asg(0)
gotDouble = adbl(0)
gotCurrency = acur(0)
gotDate = adate(0)
gotString = astr(0)
End Sub

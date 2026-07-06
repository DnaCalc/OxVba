Sub Main()
Dim longLongValue As LongLong
Dim integerValue As Integer
Dim byteValue As Byte
Dim boolValue As Boolean
Dim currencyValue As Currency
Dim singleValue As Single
Dim doubleValue As Double
Dim dateValue As Date
Dim marker
longLongValue = 1111111111^
integerValue = 12%
byteValue = CByte(3)
boolValue = False
currencyValue = 1.2345@
singleValue = 4.5!
doubleValue = 6.75#
dateValue = CDate(2#)
Call Mutate(target := marker, ByVal longLongValue, ByVal integerValue, ByVal byteValue, ByVal boolValue, ByVal currencyValue, ByVal singleValue, ByVal doubleValue, ByVal dateValue)
Dim afterLongLong As LongLong
Dim afterInteger As Integer
Dim afterByte As Byte
Dim afterBool As Boolean
Dim afterCurrency As Currency
Dim afterSingle As Single
Dim afterDouble As Double
Dim afterDate As Date
afterLongLong = longLongValue
afterInteger = integerValue
afterByte = byteValue
afterBool = boolValue
afterCurrency = currencyValue
afterSingle = singleValue
afterDouble = doubleValue
afterDate = dateValue
End Sub

Sub Mutate(ByRef target, ParamArray items() As Variant)
items(0) = 5000000012^
items(1) = 99%
items(2) = CByte(7)
items(3) = True
items(4) = 12.3456@
items(5) = 1.25!
items(6) = 2.5#
items(7) = CDate(36527#)
target = UBound(items) + 101
End Sub

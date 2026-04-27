Sub Main()
Dim a
Dim b
Dim c
Dim d
Dim e
a = Year(DateValue("1 Jan 2000"))
b = Month(CDate("Jan. 1, 2000"))
c = Day(DateValue("January 1, 2000"))
d = IsDate("1 Jan 2000")
e = IsDate("February 30, 2000")
End Sub

Declare Sub HostNoop Lib "host" Alias "noop" (ByVal x As Long)

Sub Main()
    Dim y
    y = 4
    Call HostNoop(y)
    y = y + 1
End Sub

Declare Function HostPing Lib "host" Alias "ping" (ByVal x As Long) As Long

Sub Main()
    Dim y
    y = HostPing(3)
End Sub

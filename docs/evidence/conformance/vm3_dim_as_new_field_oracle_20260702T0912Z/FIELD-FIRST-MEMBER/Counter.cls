VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Counter"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = True
Option Explicit
Private n As Long

Private Sub Class_Initialize()
    n = 10
    Main.Log = Main.Log & "I;"
End Sub

Private Sub Class_Terminate()
    Main.Log = Main.Log & "T" & CStr(n) & ";"
End Sub

Public Sub Bump()
    n = n + 1
End Sub

Public Property Get Total() As Long
    Total = n
End Property

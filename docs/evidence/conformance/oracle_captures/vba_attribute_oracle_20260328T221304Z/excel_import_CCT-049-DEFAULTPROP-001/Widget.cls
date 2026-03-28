VERSION 1.0 CLASS
BEGIN
  MultiUse = -1  'True
END
Attribute VB_Name = "Widget"
Attribute VB_GlobalNameSpace = False
Attribute VB_Creatable = False
Attribute VB_PredeclaredId = False
Attribute VB_Exposed = False
Option Explicit
Private stored As Long

Public Sub Class_Initialize()
    stored = 41
End Sub

Public Property Get Value() As Long
    Value = stored + 1
End Property
Attribute Value.VB_UserMemId = 0

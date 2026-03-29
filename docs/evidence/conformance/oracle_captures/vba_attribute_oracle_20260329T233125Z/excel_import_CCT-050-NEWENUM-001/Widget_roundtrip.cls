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
Private items As New Collection

Public Sub Class_Initialize()
    items.Add 41
    items.Add 42
End Sub

Public Property Get NewEnum() As IUnknown
    Set NewEnum = items.[_NewEnum]
End Property

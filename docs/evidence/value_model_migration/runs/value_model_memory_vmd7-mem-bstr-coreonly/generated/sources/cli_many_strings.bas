Option Explicit
Public Sub Main()
    Dim i As Long
    Dim total As Long
    Dim joined As String
    Dim parts As Variant
    joined =         "p0001xxxxxxxxxxxxxxxxxxx|p0002xxxxxxxxxxxxxxxxxxx|p0003xxxxxxxxxxxxxxxxxxx|p0004xxxxxxxxxxxxxxxxxxx|p0005xxxxxxxxxxxxxxx" & _
        "xxxx|p0006xxxxxxxxxxxxxxxxxxx|p0007xxxxxxxxxxxxxxxxxxx|p0008xxxxxxxxxxxxxxxxxxx|p0009xxxxxxxxxxxxxxxxxxx|p0010xxxxxxxxxx" & _
        "xxxxxxxxx|p0011xxxxxxxxxxxxxxxxxxx|p0012xxxxxxxxxxxxxxxxxxx|p0013xxxxxxxxxxxxxxxxxxx|p0014xxxxxxxxxxxxxxxxxxx|p0015xxxxx" & _
        "xxxxxxxxxxxxxx|p0016xxxxxxxxxxxxxxxxxxx|p0017xxxxxxxxxxxxxxxxxxx|p0018xxxxxxxxxxxxxxxxxxx|p0019xxxxxxxxxxxxxxxxxxx|p0020" & _
        "xxxxxxxxxxxxxxxxxxx|p0021xxxxxxxxxxxxxxxxxxx|p0022xxxxxxxxxxxxxxxxxxx|p0023xxxxxxxxxxxxxxxxxxx|p0024xxxxxxxxxxxxxxxxxxx|" & _
        "p0025xxxxxxxxxxxxxxxxxxx|p0026xxxxxxxxxxxxxxxxxxx|p0027xxxxxxxxxxxxxxxxxxx|p0028xxxxxxxxxxxxxxxxxxx|p0029xxxxxxxxxxxxxxx" & _
        "xxxx|p0030xxxxxxxxxxxxxxxxxxx|p0031xxxxxxxxxxxxxxxxxxx|p0032xxxxxxxxxxxxxxxxxxx|p0033xxxxxxxxxxxxxxxxxxx|p0034xxxxxxxxxx" & _
        "xxxxxxxxx|p0035xxxxxxxxxxxxxxxxxxx|p0036xxxxxxxxxxxxxxxxxxx|p0037xxxxxxxxxxxxxxxxxxx|p0038xxxxxxxxxxxxxxxxxxx|p0039xxxxx" & _
        "xxxxxxxxxxxxxx|p0040xxxxxxxxxxxxxxxxxxx|p0041xxxxxxxxxxxxxxxxxxx|p0042xxxxxxxxxxxxxxxxxxx|p0043xxxxxxxxxxxxxxxxxxx|p0044" & _
        "xxxxxxxxxxxxxxxxxxx|p0045xxxxxxxxxxxxxxxxxxx|p0046xxxxxxxxxxxxxxxxxxx|p0047xxxxxxxxxxxxxxxxxxx|p0048xxxxxxxxxxxxxxxxxxx|" & _
        "p0049xxxxxxxxxxxxxxxxxxx|p0050xxxxxxxxxxxxxxxxxxx|p0051xxxxxxxxxxxxxxxxxxx|p0052xxxxxxxxxxxxxxxxxxx|p0053xxxxxxxxxxxxxxx" & _
        "xxxx|p0054xxxxxxxxxxxxxxxxxxx|p0055xxxxxxxxxxxxxxxxxxx|p0056xxxxxxxxxxxxxxxxxxx|p0057xxxxxxxxxxxxxxxxxxx|p0058xxxxxxxxxx" & _
        "xxxxxxxxx|p0059xxxxxxxxxxxxxxxxxxx|p0060xxxxxxxxxxxxxxxxxxx|p0061xxxxxxxxxxxxxxxxxxx|p0062xxxxxxxxxxxxxxxxxxx|p0063xxxxx" & _
        "xxxxxxxxxxxxxx|p0064xxxxxxxxxxxxxxxxxxx|p0065xxxxxxxxxxxxxxxxxxx|p0066xxxxxxxxxxxxxxxxxxx|p0067xxxxxxxxxxxxxxxxxxx|p0068" & _
        "xxxxxxxxxxxxxxxxxxx|p0069xxxxxxxxxxxxxxxxxxx|p0070xxxxxxxxxxxxxxxxxxx|p0071xxxxxxxxxxxxxxxxxxx|p0072xxxxxxxxxxxxxxxxxxx|" & _
        "p0073xxxxxxxxxxxxxxxxxxx|p0074xxxxxxxxxxxxxxxxxxx|p0075xxxxxxxxxxxxxxxxxxx|p0076xxxxxxxxxxxxxxxxxxx|p0077xxxxxxxxxxxxxxx" & _
        "xxxx|p0078xxxxxxxxxxxxxxxxxxx|p0079xxxxxxxxxxxxxxxxxxx|p0080xxxxxxxxxxxxxxxxxxx|p0081xxxxxxxxxxxxxxxxxxx|p0082xxxxxxxxxx" & _
        "xxxxxxxxx|p0083xxxxxxxxxxxxxxxxxxx|p0084xxxxxxxxxxxxxxxxxxx|p0085xxxxxxxxxxxxxxxxxxx|p0086xxxxxxxxxxxxxxxxxxx|p0087xxxxx" & _
        "xxxxxxxxxxxxxx|p0088xxxxxxxxxxxxxxxxxxx|p0089xxxxxxxxxxxxxxxxxxx|p0090xxxxxxxxxxxxxxxxxxx|p0091xxxxxxxxxxxxxxxxxxx|p0092" & _
        "xxxxxxxxxxxxxxxxxxx|p0093xxxxxxxxxxxxxxxxxxx|p0094xxxxxxxxxxxxxxxxxxx|p0095xxxxxxxxxxxxxxxxxxx|p0096xxxxxxxxxxxxxxxxxxx|" & _
        "p0097xxxxxxxxxxxxxxxxxxx|p0098xxxxxxxxxxxxxxxxxxx|p0099xxxxxxxxxxxxxxxxxxx|p0100xxxxxxxxxxxxxxxxxxx|p0101xxxxxxxxxxxxxxx" & _
        "xxxx|p0102xxxxxxxxxxxxxxxxxxx|p0103xxxxxxxxxxxxxxxxxxx|p0104xxxxxxxxxxxxxxxxxxx|p0105xxxxxxxxxxxxxxxxxxx|p0106xxxxxxxxxx" & _
        "xxxxxxxxx|p0107xxxxxxxxxxxxxxxxxxx|p0108xxxxxxxxxxxxxxxxxxx|p0109xxxxxxxxxxxxxxxxxxx|p0110xxxxxxxxxxxxxxxxxxx|p0111xxxxx" & _
        "xxxxxxxxxxxxxx|p0112xxxxxxxxxxxxxxxxxxx|p0113xxxxxxxxxxxxxxxxxxx|p0114xxxxxxxxxxxxxxxxxxx|p0115xxxxxxxxxxxxxxxxxxx|p0116" & _
        "xxxxxxxxxxxxxxxxxxx|p0117xxxxxxxxxxxxxxxxxxx|p0118xxxxxxxxxxxxxxxxxxx|p0119xxxxxxxxxxxxxxxxxxx|p0120xxxxxxxxxxxxxxxxxxx|" & _
        "p0121xxxxxxxxxxxxxxxxxxx|p0122xxxxxxxxxxxxxxxxxxx|p0123xxxxxxxxxxxxxxxxxxx|p0124xxxxxxxxxxxxxxxxxxx|p0125xxxxxxxxxxxxxxx" & _
        "xxxx|p0126xxxxxxxxxxxxxxxxxxx|p0127xxxxxxxxxxxxxxxxxxx|p0128xxxxxxxxxxxxxxxxxxx|p0129xxxxxxxxxxxxxxxxxxx|p0130xxxxxxxxxx" & _
        "xxxxxxxxx|p0131xxxxxxxxxxxxxxxxxxx|p0132xxxxxxxxxxxxxxxxxxx|p0133xxxxxxxxxxxxxxxxxxx|p0134xxxxxxxxxxxxxxxxxxx|p0135xxxxx" & _
        "xxxxxxxxxxxxxx|p0136xxxxxxxxxxxxxxxxxxx|p0137xxxxxxxxxxxxxxxxxxx|p0138xxxxxxxxxxxxxxxxxxx|p0139xxxxxxxxxxxxxxxxxxx|p0140" & _
        "xxxxxxxxxxxxxxxxxxx|p0141xxxxxxxxxxxxxxxxxxx|p0142xxxxxxxxxxxxxxxxxxx|p0143xxxxxxxxxxxxxxxxxxx|p0144xxxxxxxxxxxxxxxxxxx|" & _
        "p0145xxxxxxxxxxxxxxxxxxx|p0146xxxxxxxxxxxxxxxxxxx|p0147xxxxxxxxxxxxxxxxxxx|p0148xxxxxxxxxxxxxxxxxxx|p0149xxxxxxxxxxxxxxx" & _
        "xxxx|p0150xxxxxxxxxxxxxxxxxxx|p0151xxxxxxxxxxxxxxxxxxx|p0152xxxxxxxxxxxxxxxxxxx|p0153xxxxxxxxxxxxxxxxxxx|p0154xxxxxxxxxx" & _
        "xxxxxxxxx|p0155xxxxxxxxxxxxxxxxxxx|p0156xxxxxxxxxxxxxxxxxxx|p0157xxxxxxxxxxxxxxxxxxx|p0158xxxxxxxxxxxxxxxxxxx|p0159xxxxx" & _
        "xxxxxxxxxxxxxx|p0160xxxxxxxxxxxxxxxxxxx|p0161xxxxxxxxxxxxxxxxxxx|p0162xxxxxxxxxxxxxxxxxxx|p0163xxxxxxxxxxxxxxxxxxx|p0164" & _
        "xxxxxxxxxxxxxxxxxxx|p0165xxxxxxxxxxxxxxxxxxx|p0166xxxxxxxxxxxxxxxxxxx|p0167xxxxxxxxxxxxxxxxxxx|p0168xxxxxxxxxxxxxxxxxxx|" & _
        "p0169xxxxxxxxxxxxxxxxxxx|p0170xxxxxxxxxxxxxxxxxxx|p0171xxxxxxxxxxxxxxxxxxx|p0172xxxxxxxxxxxxxxxxxxx|p0173xxxxxxxxxxxxxxx" & _
        "xxxx|p0174xxxxxxxxxxxxxxxxxxx|p0175xxxxxxxxxxxxxxxxxxx|p0176xxxxxxxxxxxxxxxxxxx|p0177xxxxxxxxxxxxxxxxxxx|p0178xxxxxxxxxx" & _
        "xxxxxxxxx|p0179xxxxxxxxxxxxxxxxxxx|p0180xxxxxxxxxxxxxxxxxxx|p0181xxxxxxxxxxxxxxxxxxx|p0182xxxxxxxxxxxxxxxxxxx|p0183xxxxx" & _
        "xxxxxxxxxxxxxx|p0184xxxxxxxxxxxxxxxxxxx|p0185xxxxxxxxxxxxxxxxxxx|p0186xxxxxxxxxxxxxxxxxxx|p0187xxxxxxxxxxxxxxxxxxx|p0188" & _
        "xxxxxxxxxxxxxxxxxxx|p0189xxxxxxxxxxxxxxxxxxx|p0190xxxxxxxxxxxxxxxxxxx|p0191xxxxxxxxxxxxxxxxxxx|p0192xxxxxxxxxxxxxxxxxxx|" & _
        "p0193xxxxxxxxxxxxxxxxxxx|p0194xxxxxxxxxxxxxxxxxxx|p0195xxxxxxxxxxxxxxxxxxx|p0196xxxxxxxxxxxxxxxxxxx|p0197xxxxxxxxxxxxxxx" & _
        "xxxx|p0198xxxxxxxxxxxxxxxxxxx|p0199xxxxxxxxxxxxxxxxxxx|p0200xxxxxxxxxxxxxxxxxxx|p0201xxxxxxxxxxxxxxxxxxx|p0202xxxxxxxxxx" & _
        "xxxxxxxxx|p0203xxxxxxxxxxxxxxxxxxx|p0204xxxxxxxxxxxxxxxxxxx|p0205xxxxxxxxxxxxxxxxxxx|p0206xxxxxxxxxxxxxxxxxxx|p0207xxxxx" & _
        "xxxxxxxxxxxxxx|p0208xxxxxxxxxxxxxxxxxxx|p0209xxxxxxxxxxxxxxxxxxx|p0210xxxxxxxxxxxxxxxxxxx|p0211xxxxxxxxxxxxxxxxxxx|p0212" & _
        "xxxxxxxxxxxxxxxxxxx|p0213xxxxxxxxxxxxxxxxxxx|p0214xxxxxxxxxxxxxxxxxxx|p0215xxxxxxxxxxxxxxxxxxx|p0216xxxxxxxxxxxxxxxxxxx|" & _
        "p0217xxxxxxxxxxxxxxxxxxx|p0218xxxxxxxxxxxxxxxxxxx|p0219xxxxxxxxxxxxxxxxxxx|p0220xxxxxxxxxxxxxxxxxxx|p0221xxxxxxxxxxxxxxx" & _
        "xxxx|p0222xxxxxxxxxxxxxxxxxxx|p0223xxxxxxxxxxxxxxxxxxx|p0224xxxxxxxxxxxxxxxxxxx|p0225xxxxxxxxxxxxxxxxxxx|p0226xxxxxxxxxx" & _
        "xxxxxxxxx|p0227xxxxxxxxxxxxxxxxxxx|p0228xxxxxxxxxxxxxxxxxxx|p0229xxxxxxxxxxxxxxxxxxx|p0230xxxxxxxxxxxxxxxxxxx|p0231xxxxx" & _
        "xxxxxxxxxxxxxx|p0232xxxxxxxxxxxxxxxxxxx|p0233xxxxxxxxxxxxxxxxxxx|p0234xxxxxxxxxxxxxxxxxxx|p0235xxxxxxxxxxxxxxxxxxx|p0236" & _
        "xxxxxxxxxxxxxxxxxxx|p0237xxxxxxxxxxxxxxxxxxx|p0238xxxxxxxxxxxxxxxxxxx|p0239xxxxxxxxxxxxxxxxxxx|p0240xxxxxxxxxxxxxxxxxxx|" & _
        "p0241xxxxxxxxxxxxxxxxxxx|p0242xxxxxxxxxxxxxxxxxxx|p0243xxxxxxxxxxxxxxxxxxx|p0244xxxxxxxxxxxxxxxxxxx|p0245xxxxxxxxxxxxxxx" & _
        "xxxx|p0246xxxxxxxxxxxxxxxxxxx|p0247xxxxxxxxxxxxxxxxxxx|p0248xxxxxxxxxxxxxxxxxxx|p0249xxxxxxxxxxxxxxxxxxx|p0250xxxxxxxxxx" & _
        "xxxxxxxxx|p0251xxxxxxxxxxxxxxxxxxx|p0252xxxxxxxxxxxxxxxxxxx|p0253xxxxxxxxxxxxxxxxxxx|p0254xxxxxxxxxxxxxxxxxxx|p0255xxxxx" & _
        "xxxxxxxxxxxxxx|p0256xxxxxxxxxxxxxxxxxxx"
    For i = 1 To 120
        parts = Split(joined, "|")
        total = total + Len(Join(parts, ""))
    Next i
End Sub

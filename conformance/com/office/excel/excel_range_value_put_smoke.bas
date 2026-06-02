Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet
    Dim cell
    Dim cells
    Dim found

    app = CreateObject("Excel.Application")
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheet = DispatchInvoke(book, "Worksheets", 1)
    cell = DispatchInvoke(sheet, "Range", "A1")
    cell.Value = "needle"
    cells = DispatchInvoke(sheet, "Cells")
    found = DispatchInvoke(cells, "Find", What:="needle", LookIn:=-4163, LookAt:=1)
    DispatchInvoke(book, "Close", False)
    DispatchInvoke(app, "Quit")
End Sub

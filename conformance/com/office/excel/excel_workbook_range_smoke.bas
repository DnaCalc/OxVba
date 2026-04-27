Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet
    Dim cell
    Dim observed

    app = CreateObject("Excel.Application")
    DispatchInvoke(app, "DisplayAlerts", False)
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheet = DispatchInvoke(book, "Worksheets", 1)
    cell = DispatchInvoke(sheet, "Range", "A1")
    DispatchInvoke(cell, "Value", 42)
    observed = DispatchInvoke(DispatchInvoke(sheet, "Range", "A1"), "Value")
    DispatchInvoke(book, "Close", False)
    DispatchInvoke(app, "Quit")
End Sub

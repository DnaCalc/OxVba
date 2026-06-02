Sub Main()
    Dim app
    Dim books
    Dim book
    Dim sheet

    app = CreateObject("Excel.Application")
    books = DispatchInvoke(app, "Workbooks")
    book = DispatchInvoke(books, "Add")
    sheet = DispatchInvoke(book, "Worksheets", 1)
    book.Close False
    app.Quit
End Sub

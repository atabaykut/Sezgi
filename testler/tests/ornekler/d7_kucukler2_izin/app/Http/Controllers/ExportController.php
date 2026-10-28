<?php

namespace App\Http\Controllers;

use App\Models\Book;
use App\Permissions\Permission;

class ExportController extends Controller
{
    public function __construct()
    {
        $this->middleware(Permission::ContentExport->middleware());
    }

    public function show(Book $book)
    {
        return $book;
    }
}

<?php

namespace App\Http\Controllers;

use App\Models\Book;
use App\Permissions\Permission;
use Illuminate\Http\Request;

class BookController extends Controller
{
    public function index()
    {
        return Book::all();
    }

    public function store(Request $request)
    {
        $this->checkPermission(Permission::BookCreateAll);
        $sifreli = $request->has('locked');
        $request->validate([
            'name'     => ['required', 'string', 'max:255'],
            'image'    => array_merge(['nullable'], $this->getImageValidationRules()),
            'password' => $sifreli ? ['required', 'string'] : null,
        ]);

        return Book::create($request->all());
    }

    public function update(Request $request, Book $book)
    {
        $this->checkOwnablePermission(Permission::BookUpdate, $book);
        $book->update($request->all());

        return $book;
    }
}

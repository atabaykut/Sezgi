<?php

namespace App\Http\Controllers;

use App\Models\Link;
use App\Models\Note;
use App\Repositories\TrashRepository;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;

class TrashController extends Controller
{
    public function index()
    {
        return view('trash.index', ['links' => Link::onlyTrashed()->get()]);
    }

    public function clearTrash(Request $request): RedirectResponse
    {
        TrashRepository::delete($request->input('model'));

        return redirect()->route('get-trash');
    }

    public function purgeNotes(Request $request): RedirectResponse
    {
        Note::where('user_id', $request->user()->id)->delete();

        return redirect()->route('get-trash');
    }

    public function clearDoneNotes(Request $request): RedirectResponse
    {
        Note::where('done', true)
            ->where('user_id', $request->user()->id)
            ->delete();

        return redirect()->route('get-trash');
    }

    public function purgeLinks(Request $request): RedirectResponse
    {
        Link::where('user_id', $request->user()->id)->delete();

        return redirect()->route('get-trash');
    }
}

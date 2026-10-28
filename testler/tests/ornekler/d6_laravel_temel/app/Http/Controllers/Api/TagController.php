<?php

namespace App\Http\Controllers\Api;

use App\Enums\TagKind;
use App\Http\Controllers\Controller;
use App\Models\Tag;
use Illuminate\Http\Request;
use Illuminate\Validation\Rule;

class TagController extends Controller
{
    public function index()
    {
        return Tag::all();
    }

    public function show(Tag $tag)
    {
        return $tag;
    }

    public function update(Request $request, Tag $tag)
    {
        $request->validate([
            'name' => ['required', 'string', Rule::unique('tags')->ignore($tag->id)],
            'kind' => ['required', Rule::enum(TagKind::class)],
            'color' => ['nullable', Rule::in(['red', 'blue'])],
        ]);
        $tag->update($request->only(['name', 'kind', 'color']));

        return $tag;
    }
}

<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Models\Comment;
use App\Models\Post;
use Illuminate\Http\Request;

class CommentController extends Controller
{
    public function index(Post $post)
    {
        return $post->comments()->get();
    }

    public function store(Request $request, Post $post)
    {
        $data = $request->validate([
            'body' => ['required', 'string', 'min:3'],
            'rating' => ['nullable', 'integer', 'min:1', 'max:5'],
            'meta.source' => 'required|in:web,mobile',
            'meta.lang' => 'string|size:2',
            'attachments' => 'array|max:3',
            'attachments.*.url' => 'required|url',
            'labels.*' => 'string|max:20',
            'avatar' => 'nullable|image|max:2048',
            'author_id' => 'required|exists:users,id',
            'code' => ['required', new \App\Rules\Uppercase],
        ]);

        return $post->comments()->create($data);
    }
}

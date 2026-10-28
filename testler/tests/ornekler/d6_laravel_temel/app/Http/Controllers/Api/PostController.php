<?php

namespace App\Http\Controllers\Api;

use App\Http\Controllers\Controller;
use App\Http\Requests\StorePostRequest;
use App\Http\Requests\UpdatePostRequest;
use App\Http\Resources\PostCollection;
use App\Http\Resources\PostResource;
use App\Models\Post;
use App\Services\PostService;
use Illuminate\Http\Request;

class PostController extends Controller
{
    public function __construct(private PostService $posts)
    {
    }

    /**
     * Gönderileri listeler.
     */
    public function index(Request $request)
    {
        $request->validate([
            'q' => 'nullable|string|max:100',
            'per_page' => 'sometimes|integer|between:1,100',
        ]);

        return new PostCollection(Post::with('author')->latest()->paginate());
    }

    /**
     * Yeni gönderi oluşturur.
     */
    public function store(StorePostRequest $request)
    {
        $post = $request->user()->posts()->create($request->validated());
        $post->tags()->sync($request->input('tags', []));

        return (new PostResource($post))->response()->setStatusCode(201);
    }

    public function show(Post $post)
    {
        return new PostResource($post->load('comments'));
    }

    public function update(UpdatePostRequest $request, Post $post)
    {
        $this->authorize('update', $post);
        $post->update($request->validated());

        return PostResource::make($post);
    }

    public function destroy(Request $request, Post $post)
    {
        if ($post->user_id !== $request->user()->id) {
            abort(403, 'Bu gönderi sizin değil');
        }
        $this->posts->remove($post);

        return response()->noContent();
    }

    public function forceDestroy(int $post)
    {
        $model = Post::withTrashed()->findOrFail($post);
        $model->forceDelete();

        return response()->json(['deleted' => true]);
    }
}

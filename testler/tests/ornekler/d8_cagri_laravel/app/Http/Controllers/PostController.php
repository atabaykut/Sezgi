<?php

namespace App\Http\Controllers;

use App\Models\Post;
use App\Services\AuditService;
use App\Services\PostService;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;

class PostController extends Controller
{
    public function __construct(private PostService $posts)
    {
    }

    public function update(Request $request, Post $post)
    {
        $this->authorize('update', $post);
        return $this->posts->rename($post, $request->input('title'));
    }

    public function destroy(Request $request, Post $post)
    {
        if ($post->user_id !== $request->user()->id) {
            abort(403);
        }
        app(AuditService::class)->record('post.destroy');
        $post->delete();
        return response()->noContent();
    }

    public function stats()
    {
        return DB::table('post_stats')->get();
    }
}

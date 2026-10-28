<?php

namespace App\Services;

use App\Events\PostRemoved;
use App\Models\Post;

class PostService
{
    public function remove(Post $post): void
    {
        $post->comments()->delete();
        $post->delete();
        PostRemoved::dispatch($post->id);
    }
}

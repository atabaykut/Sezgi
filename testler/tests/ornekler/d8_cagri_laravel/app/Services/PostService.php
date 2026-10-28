<?php

namespace App\Services;

use App\Models\Post;

class PostService
{
    public function rename(Post $post, string $title): Post
    {
        $post->title = $title;
        $post->save();
        return $post;
    }
}

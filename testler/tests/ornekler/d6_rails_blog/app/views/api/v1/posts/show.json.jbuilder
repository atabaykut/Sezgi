json.partial! "api/v1/posts/post", post: @post
json.comments @post.comments, :id, :body

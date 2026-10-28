json.extract! post, :id, :title, :body, :status, :created_at
json.author do
  json.name post.user.name
end
json.url api_v1_post_url(post, format: :json)

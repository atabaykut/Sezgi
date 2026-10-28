class Api::V1::CommentsController < Api::V1::BaseController
  skip_before_action :authenticate_with_token, only: :index

  def index
    @comments = Post.find(params[:post_id]).comments
    render json: @comments
  end

  def create
    @post = Post.find(params[:post_id])
    @comment = @post.comments.create!(params.require(:comment).permit(:body))
    render json: @comment, status: :created
  end
end

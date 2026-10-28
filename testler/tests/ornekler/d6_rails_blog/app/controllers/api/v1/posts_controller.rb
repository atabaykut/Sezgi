module Api
  module V1
    class PostsController < BaseController
      include Pundit::Authorization
      skip_before_action :authenticate_with_token, only: [:index, :show, :search]
      before_action :set_post, only: [:show, :update, :destroy, :publish]

      # Yayımlanmış yazılar
      def index
        @posts = Post.where(published: true).order(created_at: :desc)
      end

      def show
      end

      def search
        @posts = Post.where("title LIKE ?", "%#{params[:q]}%").limit(params.fetch(:limit, 20))
        render :index
      end

      def create
        @post = current_user.posts.build(post_params)
        if @post.save
          PostMailer.with(post: @post).yeni_yazi.deliver_later
          render :show, status: :created
        else
          render json: @post.errors, status: :unprocessable_entity
        end
      end

      def update
        authorize @post
        if @post.update(post_params)
          render :show
        else
          render json: @post.errors, status: :unprocessable_entity
        end
      end

      def destroy
        authorize @post
        @post.destroy
        head :no_content
      end

      def publish
        authorize @post, :publish?
        @post.update!(published: params[:force] ? true : @post.ready?)
        render json: @post
      end

      private

      def set_post
        @post = Post.find(params[:id])
      end

      def post_params
        params.require(:post).permit(:title, :body, :status, tag_ids: [], meta: [:keywords, :description])
      end
    end
  end
end

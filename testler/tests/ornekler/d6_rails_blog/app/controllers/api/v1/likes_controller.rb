module Api
  module V1
    class LikesController < BaseController
      def create
        like = Post.find(params[:post_id]).likes.create!(user: current_user)
        render json: { id: like.id }, status: :created
      end

      def destroy
        Like.find(params[:id]).destroy
        head :no_content
      end
    end
  end
end

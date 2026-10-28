class CommentsController < ApplicationController
  load_and_authorize_resource

  def destroy
    @comment.destroy
    redirect_to root_path
  end
end

class ArticlesController < ApplicationController
  before_action :set_article, only: [:update, :destroy]

  def update
    if @article.user_id != @current_user.id
      head :forbidden
      return
    end
    @article.update(article_params)
    render json: @article
  end

  def destroy
    ArticleCleanup.new(@article).call
    @article.destroy
    head :no_content
  end

  private

  def set_article
    @article = Article.find(params[:id])
  end

  def article_params
    params.require(:article).permit(:title)
  end
end

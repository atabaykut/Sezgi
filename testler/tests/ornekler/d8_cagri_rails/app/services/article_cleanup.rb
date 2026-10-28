class ArticleCleanup
  def initialize(article)
    @article = article
  end

  def call
    Comment.where(article_id: @article.id).delete_all
  end
end

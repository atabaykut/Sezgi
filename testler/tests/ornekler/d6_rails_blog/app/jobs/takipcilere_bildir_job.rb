class TakipcilereBildirJob < ApplicationJob
  queue_as :default

  def perform(post_id)
    post = Post.find(post_id)
    PostMailer.with(post: post).yeni_yazi.deliver_later
  end
end

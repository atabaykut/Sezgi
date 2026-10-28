class WebhookIslemeJob < ApplicationJob
  def perform(kaynak, govde)
    Like.where(post_id: nil).delete_all
  end
end

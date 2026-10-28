class WebhooksController < ActionController::API
  def receive
    WebhookIslemeJob.perform_later(params[:kaynak], request.raw_post)
    head :accepted
  end
end

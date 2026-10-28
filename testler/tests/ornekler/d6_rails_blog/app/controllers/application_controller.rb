class ApplicationController < ActionController::Base
  include Pundit::Authorization
  before_action :authenticate_user!
  rescue_from Pundit::NotAuthorizedError, with: :yetkisiz

  private

  def yetkisiz
    head :forbidden
  end
end

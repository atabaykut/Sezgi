class ApplicationController < ActionController::API
  before_action :authenticate_user!

  def authenticate_user!
    @current_user = User.find_by(token: request.headers["Authorization"])
    head :unauthorized unless @current_user
  end
end

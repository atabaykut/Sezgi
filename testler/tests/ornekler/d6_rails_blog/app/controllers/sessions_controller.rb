class SessionsController < ApplicationController
  include Authentication
  skip_before_action :authenticate_user!
  allow_unauthenticated_access only: %i[new create]

  def new
  end

  def create
    if user = User.authenticate_by(params.permit(:email_address, :password))
      start_new_session_for user
      redirect_to root_path
    else
      redirect_to new_session_path, alert: "Try another email address or password."
    end
  end

  def destroy
    terminate_session
    redirect_to new_session_path
  end
end

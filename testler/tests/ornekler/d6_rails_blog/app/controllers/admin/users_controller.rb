module Admin
  class UsersController < ApplicationController
    before_action :require_admin

    def index
      @users = User.all
      render json: @users
    end

    def show
      render json: User.find(params[:id])
    end

    def create
      user = User.create!(user_params)
      render json: user, status: :created
    end

    def update
      user = User.find(params[:id])
      user.update!(user_params)
      render json: user
    end

    def destroy
      User.find(params[:id]).destroy!
      head :no_content
    end

    private

    def require_admin
      redirect_to root_path, alert: "Yetkisiz" unless current_user.admin?
    end

    def user_params
      params.require(:user).permit(:email, :name, :role)
    end
  end
end

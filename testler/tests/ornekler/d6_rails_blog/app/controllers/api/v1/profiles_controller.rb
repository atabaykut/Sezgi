module Api::V1
  class ProfilesController < BaseController
    def show
      render json: current_user, serializer: UserSerializer
    end

    def update
      current_user.update(params.require(:user).permit(:name, :bio, :avatar))
      render json: { ok: true, user_id: current_user.id }
    end
  end
end

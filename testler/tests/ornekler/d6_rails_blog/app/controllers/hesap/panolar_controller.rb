class Hesap::PanolarController < ApplicationController
  def ozet
    render json: { ad: current_user.name, yazi: current_user.posts.count }
  end
end

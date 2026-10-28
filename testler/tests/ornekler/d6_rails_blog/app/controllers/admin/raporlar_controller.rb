class Admin::RaporlarController < ApplicationController
  def goster
    render json: { toplam: Post.count }
  end
end

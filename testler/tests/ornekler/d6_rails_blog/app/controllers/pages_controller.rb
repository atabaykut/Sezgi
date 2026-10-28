class PagesController < ApplicationController
  skip_before_action :authenticate_user!

  def home
  end

  # Hakkımızda sayfası
  def about
    @sayi = Post.count
  end
end

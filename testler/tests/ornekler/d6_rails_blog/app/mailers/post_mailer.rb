class PostMailer < ApplicationMailer
  def yeni_yazi
    @post = params[:post]
    mail(to: @post.user.email, subject: "Yeni yazı")
  end
end

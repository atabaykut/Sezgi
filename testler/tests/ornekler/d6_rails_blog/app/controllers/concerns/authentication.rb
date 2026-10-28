module Authentication
  extend ActiveSupport::Concern

  included do
    before_action :require_authentication
  end

  class_methods do
    def allow_unauthenticated_access(**options)
      skip_before_action :require_authentication, **options
    end
  end

  private

  def require_authentication
    resume_session || request_authentication
  end

  def resume_session
    Current.session ||= Session.find_by(id: cookies.signed[:session_id])
  end

  def request_authentication
    redirect_to new_session_path
  end

  def start_new_session_for(user)
    user.sessions.create!(user_agent: request.user_agent).tap { |s| Current.session = s }
  end

  def terminate_session
    Current.session.destroy
  end
end

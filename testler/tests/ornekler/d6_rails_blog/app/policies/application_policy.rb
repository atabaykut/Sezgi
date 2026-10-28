class ApplicationPolicy
  attr_reader :user, :record

  def initialize(user, record)
    raise Pundit::NotAuthorizedError, "giriş gerekli" unless user

    @user = user
    @record = record
  end

  def update?
    false
  end

  def destroy?
    false
  end
end

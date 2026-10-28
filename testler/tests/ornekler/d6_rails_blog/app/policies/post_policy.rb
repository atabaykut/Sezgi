class PostPolicy < ApplicationPolicy
  def update?
    user.admin? || record.user_id == user.id
  end

  def destroy?
    user.admin?
  end

  def publish?
    user.editor? || user.admin?
  end
end

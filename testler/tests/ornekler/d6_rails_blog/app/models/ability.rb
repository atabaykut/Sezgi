class Ability
  include CanCan::Ability

  def initialize(user)
    return unless user

    can :read, :all
    if user.admin?
      can :manage, :all
    else
      can :create, Comment
    end
  end
end

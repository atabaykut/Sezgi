class User < ApplicationRecord
  devise :database_authenticatable, :registerable,
         :recoverable, :rememberable, :validatable

  has_many :posts, dependent: :destroy
  has_many :comments, dependent: :nullify
  has_many :sessions, dependent: :destroy
  has_one_attached :avatar

  enum :role, { member: 0, editor: 1, admin: 2 }

  validates :name, length: { maximum: 50 }
end

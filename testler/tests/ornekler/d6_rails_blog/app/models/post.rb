class Post < ApplicationRecord
  belongs_to :user
  has_many :comments, dependent: :destroy
  has_many :likes, dependent: :delete_all
  has_and_belongs_to_many :tags

  enum :status, { draft: "draft", published: "published", archived: "archived" }

  validates :title, presence: true, length: { maximum: 120 }
  validates :body, presence: true
  validate :yasakli_kelime_yok

  after_create_commit { TakipcilereBildirJob.perform_later(id) }
  before_destroy :arsivle

  def ready?
    body.present?
  end

  private

  def arsivle
    ArsivKaydi.create!(post_id: id, title: title)
  end

  def yasakli_kelime_yok
    errors.add(:body, "yasaklı") if body&.include?("yasak")
  end
end

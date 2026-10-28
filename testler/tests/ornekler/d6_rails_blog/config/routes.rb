Rails.application.routes.draw do
  devise_for :users

  root "pages#home"
  get "about", to: "pages#about"
  get "up" => "rails/health#show", as: :rails_health_check

  concern :commentable do
    resources :comments, only: [:index, :create]
  end

  namespace :api do
    namespace :v1 do
      resources :posts, concerns: :commentable do
        member do
          post :publish
        end
        collection do
          get :search
        end
        resources :likes, only: [:create, :destroy], shallow: true
      end
      resource :profile, only: %i[show update]
    end
  end

  resources :comments, only: [:destroy]
  resource :session, only: [:new, :create, :destroy]

  scope "/hesap", module: "hesap" do
    get "ozet", to: "panolar#ozet"
  end

  constraints subdomain: "admin" do
    draw :admin
  end

  if Rails.env.development?
    mount LetterOpenerWeb::Engine, at: "/letter_opener"
  end

  match "webhooks/:kaynak", to: "webhooks#receive", via: [:post, :put]
  get "/eski", to: redirect("/yeni")

  %w[gizlilik kosullar].each do |sayfa|
    get sayfa, to: "pages##{sayfa}"
  end
end

namespace :admin do
  resources :users, except: [:new, :edit]
  get "rapor", to: "raporlar#goster"
end

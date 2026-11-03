Rails.application.routes.draw do
  get "raporlar", to: "raporlar#index"
  get "raporlar/donusum", to: "raporlar#donusum"
  get "raporlar/bos", to: "raporlar#bos"
end

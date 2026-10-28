import { useSuspenseQuery } from "@tanstack/react-query"
import { createFileRoute, useNavigate } from "@tanstack/react-router"

import { ItemsService } from "@/client"
import AddItem from "@/components/Items/AddItem"
import DeleteItem from "@/components/Items/DeleteItem"

function getItemsQueryOptions() {
  return {
    queryFn: () => ItemsService.readItems({ query: { skip: 0, limit: 100 } }),
    queryKey: ["items"],
  }
}

export const Route = createFileRoute("/_layout/items")({
  component: Items,
})

function Items() {
  const navigate = useNavigate()
  const { data } = useSuspenseQuery(getItemsQueryOptions())
  // Yönlendirme ve çeviri çağrıları API çağrısı değildir (yanlış pozitif denetimi)
  const git = () => navigate({ to: "/items" })
  const baslik = t("/items/title")
  return (
    <div>
      <h1>{baslik}</h1>
      <AddItem />
      {data.data.map((item) => (
        <DeleteItem key={item.id} id={item.id} />
      ))}
      <a href="/items" onClick={git}>Items</a>
    </div>
  )
}

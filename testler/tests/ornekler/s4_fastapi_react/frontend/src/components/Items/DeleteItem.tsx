import { useMutation } from "@tanstack/react-query"

import { ItemsService } from "@/client"
import { Button } from "@/components/ui/button"

const DeleteItem = ({ id }: { id: string }) => {
  const deleteItem = async (id: string) => {
    await ItemsService.deleteItem({ path: { id } })
  }

  const mutation = useMutation({
    mutationFn: deleteItem,
  })

  return (
    <Button variant="destructive" aria-label="Delete item" onClick={() => mutation.mutate(id)}>
      <TrashIcon />
    </Button>
  )
}

export default DeleteItem

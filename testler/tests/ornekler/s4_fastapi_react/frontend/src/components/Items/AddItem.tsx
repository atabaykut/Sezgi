import { useMutation, useQueryClient } from "@tanstack/react-query"
import { useState } from "react"
import { useForm } from "react-hook-form"

import { type ItemCreate, ItemsService } from "@/client"
import { Button } from "@/components/ui/button"
import { LoadingButton } from "@/components/ui/loading-button"

const AddItem = () => {
  const [isOpen, setIsOpen] = useState(false)
  const queryClient = useQueryClient()
  const form = useForm<ItemCreate>()

  const mutation = useMutation({
    mutationFn: (data: ItemCreate) => ItemsService.createItem({ body: data }),
    onSuccess: () => {
      form.reset()
      setIsOpen(false)
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ["items"] })
    },
  })

  const onSubmit = (data: ItemCreate) => {
    mutation.mutate(data)
  }

  return (
    <div>
      <Button className="my-4" onClick={() => setIsOpen(true)}>
        Add Item
      </Button>
      <form onSubmit={form.handleSubmit(onSubmit)}>
        <input placeholder="Title" {...form.register("title")} />
        <Button variant="outline" onClick={() => setIsOpen(false)}>
          Cancel
        </Button>
        <LoadingButton type="submit" loading={mutation.isPending}>
          Save
        </LoadingButton>
      </form>
    </div>
  )
}

export default AddItem

import { Table, Button } from 'react-bootstrap';
import { FaTrash } from 'react-icons/fa';
import { useGetProductsQuery, useDeleteProductMutation, useCreateProductMutation } from '../../slices/productsApiSlice';

const ProductListScreen = () => {
  const { data, isLoading, error, refetch } = useGetProductsQuery({ pageNumber: 1 });
  const [deleteProduct, { isLoading: loadingDelete }] = useDeleteProductMutation();

  const deleteHandler = async (id) => {
    if (window.confirm('Are you sure')) {
      await deleteProduct(id);
      refetch();
    }
  };

  const [createProduct, { isLoading: loadingCreate }] = useCreateProductMutation();

  const createProductHandler = async () => {
    if (window.confirm('Are you sure you want to create a new product?')) {
      await createProduct();
      refetch();
    }
  };

  return (
    <>
      <Button className='my-3' onClick={createProductHandler}>
        Create Product
      </Button>
      <Table>
        {data.products.map((product) => (
          <tr key={product._id}>
            <td>{product.name}</td>
            <td>
              <Button variant='danger' className='btn-sm' onClick={() => deleteHandler(product._id)}>
                <FaTrash style={{ color: 'white' }} />
              </Button>
            </td>
          </tr>
        ))}
      </Table>
    </>
  );
};

export default ProductListScreen;

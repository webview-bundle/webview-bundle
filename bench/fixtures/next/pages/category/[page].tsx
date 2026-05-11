import { Layout, Page } from '@vercel/examples-ui';
import type { GetStaticPaths, GetStaticProps, GetStaticPropsContext } from 'next';
import Head from 'next/head';
import PaginationPage from '../../components/PaginatedPage';
import getProducts from '../../lib/getProducts';

type PageProps = {
  products: any[];
  currentPage: number;
  totalProducts: number;
};

export const PER_PAGE = 10;

function PaginatedPage({ products, currentPage, totalProducts }: PageProps) {
  return (
    <Page>
      <Head>
        <meta name="description" content={`Statically generated page ${currentPage}`} />
        <link rel="icon" href="/favicon.ico" />
      </Head>
      <PaginationPage
        products={products}
        currentPage={currentPage}
        totalProducts={totalProducts}
        perPage={PER_PAGE}
      />
    </Page>
  );
}

PaginatedPage.Layout = Layout;

export const getStaticProps: GetStaticProps = async ({ params }: GetStaticPropsContext) => {
  const page = Number(params?.page) || 1;
  const { products, total } = await getProducts({ limit: PER_PAGE, page });

  if (!products.length) {
    return {
      notFound: true,
    };
  }

  return {
    props: {
      products,
      totalProducts: total,
      currentPage: page,
    },
  };
};

export const getStaticPaths: GetStaticPaths = async () => {
  return {
    // Prerender the next 5 pages after the first page, which is handled by the index page.
    // Other pages will be prerendered at runtime.
    paths: Array.from({ length: 10 }).map((_, i) => `/category/${i + 1}`),
    // Block the request for non-generated pages and cache them in the background
    fallback: false,
  };
};

export default PaginatedPage;

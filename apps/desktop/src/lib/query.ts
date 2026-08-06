import { QueryClient } from "@tanstack/react-query"

export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // 事件轮询器会显式失效受影响的键，因此缓存视图不需要频繁被动刷新。
      staleTime: 10_000,
      gcTime: 15 * 60_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
})

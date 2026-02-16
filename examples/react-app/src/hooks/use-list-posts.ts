import { useQuery } from "@tanstack/react-query";
import useServer from "@/hooks/use-server";
import type { Post } from "@/server";

export type { Post };

export default function useListPosts() {
  const server = useServer();

  return useQuery<Post[]>({
    queryKey: ["posts"],
    queryFn: () => server!.list_posts(),
    enabled: !!server,
    structuralSharing: false,
  });
}

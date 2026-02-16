import { useQuery } from "@tanstack/react-query";
import useServer from "@/hooks/use-server";
import type { Post } from "@/server";

export default function useGetPost(id: number) {
  const server = useServer();

  return useQuery<Post>({
    queryKey: ["post", id],
    queryFn: async () => {
      const result = await server!.get_post(BigInt(id));

      if (result.__kind__ === "Err") {
        throw new Error(result.Err);
      }

      return result.Ok;
    },
    enabled: !!server && id > 0,
    structuralSharing: false,
  });
}

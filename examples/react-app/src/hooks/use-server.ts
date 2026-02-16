import { useEffect, useRef, useState } from "react";
import { HttpAgent } from "@icp-sdk/core/agent";
import { createActor, type Server } from "@/server";

const canisterId = process.env.CANISTER_ID_SERVER || "";
const isLocal = process.env.DFX_NETWORK === "local";
const host = isLocal ? "http://127.0.0.1:4943" : "https://icp-api.io";

let serverPromise: Promise<Server> | undefined;

async function initServer(): Promise<Server> {
  const agent = HttpAgent.createSync({ host });
  if (isLocal) {
    await agent.fetchRootKey();
  }
  return createActor(canisterId, { agent });
}

function getServerPromise(): Promise<Server> {
  if (!serverPromise) {
    serverPromise = initServer();
  }
  return serverPromise;
}

export default function useServer(): Server | undefined {
  const [server, setServer] = useState<Server>();
  const initiated = useRef(false);

  useEffect(() => {
    if (initiated.current) return;
    initiated.current = true;
    getServerPromise().then(setServer);
  }, []);

  return server;
}

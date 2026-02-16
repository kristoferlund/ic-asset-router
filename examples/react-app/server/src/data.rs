use candid::CandidType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, CandidType, Serialize, Deserialize)]
pub struct Post {
    pub id: i64,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub author: String,
}

pub fn all_posts() -> Vec<Post> {
    vec![
        Post {
            id: 1,
            title: "What is the Internet Computer?".into(),
            summary: "An introduction to the Internet Computer blockchain and its vision for a decentralized web.".into(),
            body: "The Internet Computer is a blockchain that runs at web speed and can serve web content directly to users. Unlike traditional blockchains that only handle tokens and simple smart contracts, the IC hosts full applications — frontend, backend, and data — entirely on-chain.\n\nThis means developers can build and deploy web applications without relying on centralized cloud providers. The result is software that is tamperproof, unstoppable, and governed by its users.".into(),
            author: "Alice".into(),
        },
        Post {
            id: 2,
            title: "Canisters: Smart Contracts Reimagined".into(),
            summary: "How canisters extend the concept of smart contracts with persistent memory and HTTP capabilities.".into(),
            body: "Canisters are the compute units of the Internet Computer. Think of them as smart contracts that can store data, serve HTTP requests, and run arbitrary WebAssembly code.\n\nEach canister has its own memory (up to hundreds of gigabytes) that persists across calls automatically — no external database needed. Canisters communicate with each other through asynchronous message passing and can be composed into complex applications.".into(),
            author: "Bob".into(),
        },
        Post {
            id: 3,
            title: "Certified Assets and Response Verification".into(),
            summary: "How the IC proves that HTTP responses are authentic without trusting a single node.".into(),
            body: "When a canister serves an HTTP response, the IC can certify that response using a Merkle tree rooted in the subnet's chain key. The client (typically a service worker or boundary node) can then verify that the response was genuinely produced by the canister.\n\nThis is what ic-asset-router helps with: it manages the certification tree, caches certified responses, and handles the query/update split transparently. The result is fast, verifiable web content served directly from the blockchain.".into(),
            author: "Charlie".into(),
        },
    ]
}

pub fn get_post(id: i64) -> Option<Post> {
    all_posts().into_iter().find(|p| p.id == id)
}

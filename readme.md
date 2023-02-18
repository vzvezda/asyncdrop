### Proof Of Concept rust runtime with async destruction

This repository contains my article "[Async destruction on stable rust](article/async-dest.md)" and the proof of concept Rust runtime with async destruction implemented as described in the article. I have published the article on r/rust to discuss with people if they think the approach is feasible, but it did not get any attention. So this repository can be useful for you if your are making a research on the topic. No future work is planned here. I have another async executor project [aiur](https://docs.rs/aiur/latest/aiur/) which can potentially evolve into something useful.

The `toy` async runtime with async destruction is implemented here:

1. It is not for production use: this is a proof of concept code that supposed to verify if the idea can actually work. While working on this library I have discovered some difficulties I did not expect initially, so it was useful.

2. To make things simple the runtime only supports futures with `()` as return type.

3. There is room for improvement runtime performance and code clarity.
4. This library uses `Arc` while for this single thread executor the `Rc` would be sufficient. The reason is uses `std::task::Wake` to implement `Waker`, which is based on `Arc`.
5. There is some unsafe internally while the public API of toy module is safe. I believe that unsafe does not produce any unsoundness. 

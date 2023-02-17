

### Proof Of Concept rust runtime with async destruction

This repository contains my article "[Async destruction on stable rust](article/async-dest.md)" and the proof of concept async runtime implementation in rust that supports the async destruction. I was published the article on r/rust to discuss if the approach is feasible, but it looks the article did not got any attention.

The `toy` async runtime with async destruction is implemented here:

2. It is not for production use: this is a proof of concept code that supposed to verify if what I had in my head can actually work. While making it I have discovered some difficulties I did not expect initially, so it was useful.

3. To make things simple the runtime only supports futures with `()` as return type.

4. There is room for improvement runtime performance and code clarity.
5. This library uses `Arc` while for this single thread executor the `Rc` would be sufficient. The reason is uses `std::task::Wake` to implement `Waker`, which is based on `Arc`.
6. There is some unsafe internally while the public API of toy module is safe. I believe that unsafe does not produce any unsoundness. 
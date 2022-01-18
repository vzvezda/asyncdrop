



1. This is the async runtime for rust that support async destruction with approach described in [my article](). 
2. It is not for production use: this is a proof of concept code that supposed to verify if what I had in my head can actually work. While making it I have discovered some difficulties I did not expect initially, so it was useful.

3. To make things easier runtime only support futures with `()` as return type.

4. There is room for improvement runtime performance or code clarity (perhaps both).
5. This library uses `Arc` while for this single thread executor the `Rc` would be sufficient. The reason is uses `std::task::Wake` to implement `Waker`, which is based on `Arc`.
6. There is some unsafe internally while the public API of toy module is safe. I believe that unsafe does not produce any unsoundness. 
# Status

There are several issues to be resolved:

- This implementation attempts structural sharing of the input and output values with minimal heap allocations. This was a lot of effort working out the lifetimes, but we are not sure it was worth it. We may consider deleting Bumpalo in the future.
- The code is too spaghetti in some places and could be more Rust-idiomatic.
- We have made a couple of optimization passes, but there are still lots of opportunities for optimization.
- There are obviously still a bunch of missing features. We're aiming for feature parity with the reference implementation wherever feasible.
- The API has not had any real thought put into it yet

# Goals

- Feature-parity with the reference implementation (within reason)
- Clean API and idiomatic code (make the easy things easy and the complex possible)
- Well-documented for users and easy to onboard for contributors
- Efficient and optimized, at least no low-hanging fruit.

There are a few other ideas that are semi-baked or non-existent:

- A command line utility and REPL (semi-baked)
- Benchmarks to track improvements within the Rust implementation and to compare against [jsonata-js](https://github.com/jsonata-js/jsonata) (non-existent).

# explore-wasm-kernel

Experiments around exploring a microkernel for WASM blobs.

## Experiments

Each experiment is a Rust project in their own directory. Compile with Cargo:

```
cd experiment
cargo build
```

And run from the target directory:

```
cd experiment/target/debug
experiment
```

### basic-mpsc-pubsub

Initial idea of having a microkernel passing pub/sub messages among worker threads.

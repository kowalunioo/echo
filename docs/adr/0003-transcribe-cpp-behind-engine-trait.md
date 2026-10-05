# transcribe-cpp behind an `Engine` trait

For 0.1.0 speech recognition uses transcribe-cpp, which runs Whisper and Parakeet GGUF models on CPU or any-vendor GPU (Vulkan). It is written by Handy's author; using it as an MIT-licensed dependency is acceptable for the clean room (it is a library we link, not code we copy) and is credited in the third-party notices. We still want to move off it eventually (backlog #1), so all recognition goes through an `Engine` trait and nothing outside the engine module depends on transcribe-cpp types.

## Considered options

- **sherpa-onnx** — rejected: its GPU path is CUDA-only, and Echo must accelerate on any GPU vendor.
- **whisper.cpp bindings directly** — no Parakeet support.

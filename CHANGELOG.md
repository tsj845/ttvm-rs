# 0.3.0
- Added support for timeouts when executing the vm

# 0.2.2
- Added ability to bind/unbind external functions to symbols in the index

# 0.2.1

- Made `VMError` implement `std::error::Error`
- Added `Memory::read_cvalue`


# 0.2.0

- Added more conversion methods to the `CValue` type
- Added doc comments
- Renamed `CValue::as_bytes` to `CValue::to_bytes` to reflect the nature of the copy


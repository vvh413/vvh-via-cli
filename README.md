# vvh-via-cli

Custom VIA CLI for [custom QMK firmware](https://github.com/vvh413/qmk_firmware/tree/wireless_playground).

Currently uses [`qmk-via-api`](https://github.com/srwi/qmk-via-api) for communication.

## TODO

- [ ] Write custom communication with HID device (using [`hidapi`](https://github.com/ruabmbua/hidapi-rs)?)
without depending on `qmk-via-api` and `pyo3`(???).
- [ ] Add custom buffer commands

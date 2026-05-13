# Manual TN3270 Smoke Test

Use this checklist when a real TN3270/TN3270E endpoint is available. Do not commit hostnames, usernames, screenshots, or packet captures that identify a real system.

## Connection

1. Start the app with `cargo run -p r3270-app`.
2. Create a profile with the endpoint host, port, TLS setting, model `IBM-3278-2-E`, and CP037 unless the host requires CP1047.
3. Connect and confirm the OIA moves from `Connecting` to `Tn3270` or `Tn3270e`.
4. Disconnect, reconnect, close the tab, and confirm the UI does not hang.

## Display And Input

1. Confirm protected labels cannot be overwritten.
2. Type into an unprotected field and press Enter.
3. Exercise PF1-PF12, Shift+PF1-PF12 for PF13-PF24, Ctrl+F1-Ctrl+F3 for PA1-PA3, Clear, Reset, Tab, Backtab, Home, arrows, Insert, Delete, and Ctrl+U.
4. Confirm colors/highlighting render correctly on screens that use extended attributes.

## File Transfer

1. Use a disposable dataset or test account only.
2. Submit an IND$FILE GET in text mode and verify host-side messages.
3. Submit an IND$FILE PUT in binary mode with a small file and verify allocation options.

## Captures

If a trace is useful for regression testing, sanitize it before adding it to the repo:

- Remove hostnames, LUs, usernames, account strings, data set names, and business data.
- Keep only the byte stream required to reproduce the rendering or parser issue.
- Add the sanitized stream as a golden test fixture with an explanation of the expected screen state.

